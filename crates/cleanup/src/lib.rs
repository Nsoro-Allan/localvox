use anyhow::{anyhow, Result};
use llama_cpp::standard_sampler::StandardSampler;
use llama_cpp::{LlamaModel, LlamaParams, SessionParams};

const SYSTEM_PROMPT: &str = "You are a transcription cleanup assistant. The user gives you a raw dictated transcript that may contain false starts and self-corrections, like saying \"go to school, sorry, to the market\" when they meant \"go to the market\". Rewrite it as clean, natural text reflecting only what the speaker meant to say. Keep their wording and tone otherwise unchanged. Do not add commentary, explanations, or quotation marks - output only the corrected text and nothing else.";

/// A loaded local LLM used to clean up disfluencies and self-corrections
/// in dictated text. Loading is the expensive part; `.clean()` on an
/// existing instance is comparatively cheap.
pub struct Cleaner {
    model: LlamaModel,
}

impl Cleaner {
    pub fn load(model_path: &str) -> Result<Self> {
        let model = LlamaModel::load_from_file(model_path, LlamaParams::default())
            .map_err(|e| anyhow!("failed to load cleanup model: {e:?}"))?;
        Ok(Self { model })
    }

    pub fn clean(&self, raw_text: &str) -> Result<String> {
        if raw_text.trim().is_empty() {
            return Ok(String::new());
        }

        let mut session = self
            .model
            .create_session(SessionParams::default())
            .map_err(|e| anyhow!("failed to create session: {e:?}"))?;

        let prompt = format!(
            "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\n{raw_text}<|im_end|>\n<|im_start|>assistant\n"
        );

        session
            .advance_context(&prompt)
            .map_err(|e| anyhow!("failed to feed prompt: {e:?}"))?;

        let max_tokens = 256;
        let completions = session
            .start_completing_with(StandardSampler::default(), max_tokens)
            .map_err(|e| anyhow!("failed to start completion: {e:?}"))?
            .into_strings();

        let mut output = String::new();
        for piece in completions {
            if piece.contains("<|im_end|>") {
                output.push_str(piece.split("<|im_end|>").next().unwrap_or(""));
                break;
            }
            output.push_str(&piece);
        }

        let cleaned = output.trim().to_string();
        if cleaned.is_empty() {
            // Fall back to the original rather than returning nothing.
            Ok(raw_text.to_string())
        } else {
            Ok(cleaned)
        }
    }
}