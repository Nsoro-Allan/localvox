use anyhow::{anyhow, Result};
use llama_cpp::standard_sampler::StandardSampler;
use llama_cpp::{LlamaModel, LlamaParams, SessionParams};

const SYSTEM_PROMPT: &str = "Clean up this dictated transcript by removing false starts and self-corrections (e.g. \"go to school, sorry, the market\" -> \"go to the market\"). Keep the rest unchanged. Output only the corrected text, nothing else.";

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

        // The model's own default context window can be tens of thousands
        // of tokens even though we only ever feed it a short prompt plus
        // one dictated utterance — right-sizing this avoids allocating a
        // KV cache far bigger than needed on every single call.
        let session_params = SessionParams {
            n_ctx: 1024,
            ..Default::default()
        };
        let mut session = self
            .model
            .create_session(session_params)
            .map_err(|e| anyhow!("failed to create session: {e:?}"))?;

        let prompt = format!(
            "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\n{raw_text}<|im_end|>\n<|im_start|>assistant\n"
        );

        session
            .advance_context(&prompt)
            .map_err(|e| anyhow!("failed to feed prompt: {e:?}"))?;

        // A cleaned utterance is never dramatically longer than the raw
        // one, so bound generation length proportionally to the input
        // instead of always paying for a flat 256-token ceiling.
        let word_count = raw_text.split_whitespace().count();
        let max_tokens = (word_count * 3 + 32).min(256);

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
            Ok(raw_text.to_string())
        } else {
            Ok(cleaned)
        }
    }
}