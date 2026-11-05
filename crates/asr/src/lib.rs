use anyhow::{anyhow, Result};
use std::path::Path;

pub mod whisper;

pub trait AsrBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn transcribe(&self, samples: &[f32], prompt: &str) -> Result<String>;
    fn is_ready(&self) -> bool {
        true
    }
}

pub fn load_backend(model_id: &str, model_path: &Path) -> Result<Box<dyn AsrBackend>> {
    let id = model_id.to_ascii_lowercase();
    if id.starts_with("whisper") || id.contains("ggml") {
        let path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow!("invalid model path"))?;
        let t = whisper::WhisperBackend::load(path_str)?;
        return Ok(Box::new(t));
    }

    Ok(Box::new(StubBackend {
        id: model_id.to_string(),
    }))
}

pub fn load_wav_as_16k_mono(path: &str) -> Result<Vec<f32>> {
    whisper::load_wav_as_16k_mono(path)
}

pub type Transcriber = whisper::WhisperBackend;

pub fn transcribe(model_path: &str, samples: &[f32]) -> Result<String> {
    let t = whisper::WhisperBackend::load(model_path)?;
    t.transcribe(samples)
}

struct StubBackend {
    id: String,
}

impl AsrBackend for StubBackend {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn is_ready(&self) -> bool {
        false
    }

    fn transcribe(&self, _samples: &[f32], _prompt: &str) -> Result<String> {
        Err(anyhow!(
            "Model '{}' backend is not implemented yet. Use a Whisper model for now.",
            self.id
        ))
    }
}
