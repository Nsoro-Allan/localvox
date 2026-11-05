//! Optional local LLM post-processing.
//!
//! Talks to a local Ollama-compatible HTTP endpoint (default
//! http://127.0.0.1:11434). Completely optional — if the endpoint is
//! unreachable the caller can fall back to the raw transcript.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessConfig {
    /// Base URL of the Ollama (or compatible) server.
    pub endpoint: String,
    /// Model name, e.g. "llama3.2" or "qwen2.5:3b".
    pub model: String,
    /// System / instruction prompt. Use `{text}` as the placeholder for the transcript.
    pub prompt_template: String,
    /// Whether post-processing is enabled.
    pub enabled: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:11434".to_string(),
            model: "llama3.2".to_string(),
            prompt_template: "Fix grammar, punctuation and filler words in the following transcript. \
Do not change the meaning or add new information. Output only the cleaned text:\n\n{text}"
                .to_string(),
            enabled: false,
        }
    }
}

#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

/// Run the transcript through a local Ollama-compatible endpoint.
pub fn process(config: &PostProcessConfig, text: &str) -> Result<String> {
    if !config.enabled {
        return Ok(text.to_string());
    }
    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let prompt = config.prompt_template.replace("{text}", text);
    let url = format!(
        "{}/api/generate",
        config.endpoint.trim_end_matches('/')
    );

    let body = OllamaGenerateRequest {
        model: &config.model,
        prompt: &prompt,
        stream: false,
    };

    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(&body)
        .with_context(|| format!("POST {url} (is Ollama running?)"))?;

    let parsed: OllamaGenerateResponse = resp
        .into_json()
        .context("parsing Ollama response")?;

    let cleaned = parsed.response.trim().to_string();
    if cleaned.is_empty() {
        return Err(anyhow!("Ollama returned empty response"));
    }
    Ok(cleaned)
}

/// Quick connectivity check.
pub fn ping(endpoint: &str) -> bool {
    let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
    ureq::get(&url).call().is_ok()
}
