use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// One entry in the model catalog. Deliberately scoped to whisper.cpp
/// GGML models for now, since that's the only runtime `asr` supports.
pub struct ModelEntry {
    pub id: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub size_mb: u64,
    /// Filled in once you've downloaded and verified a copy yourself
    /// (`sha256sum <file>`). Left as None until then rather than
    /// guessing a value, since a wrong checksum is worse than none.
    pub sha256: Option<&'static str>,
}

pub const MANIFEST: &[ModelEntry] = &[
    ModelEntry { id: "whisper-tiny-en", repo: "ggerganov/whisper.cpp", file: "ggml-tiny.en.bin", size_mb: 75, sha256: None },
    ModelEntry { id: "whisper-base-en", repo: "ggerganov/whisper.cpp", file: "ggml-base.en.bin", size_mb: 148, sha256: None },
    ModelEntry { id: "whisper-small-en", repo: "ggerganov/whisper.cpp", file: "ggml-small.en.bin", size_mb: 488, sha256: None },
    ModelEntry { id: "whisper-medium-en", repo: "ggerganov/whisper.cpp", file: "ggml-medium.en.bin", size_mb: 1500, sha256: None },
    ModelEntry { id: "whisper-large-v3-turbo", repo: "ggerganov/whisper.cpp", file: "ggml-large-v3-turbo.bin", size_mb: 1600, sha256: None },
];

pub fn find_model(id: &str) -> Option<&'static ModelEntry> {
    MANIFEST.iter().find(|m| m.id == id)
}

/// Linux: ~/.local/share/localvox/models · macOS: ~/Library/Application Support/localvox/models · Windows: %APPDATA%\localvox\models
pub fn models_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| anyhow!("could not determine OS data directory"))?;
    let dir = base.join("localvox").join("models");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("opening {path:?}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Downloads a model into the local cache and verifies it against the
/// manifest's checksum if one is set. Returns the path to the verified file.
pub fn download_model(entry: &ModelEntry) -> Result<PathBuf> {
    let dest_dir = models_dir()?;
    let dest_path = dest_dir.join(entry.file);

    if !dest_path.exists() {
        let api = hf_hub::api::sync::Api::new()?;
        let repo = api.model(entry.repo.to_string());
        let cached_path = repo
            .get(entry.file)
            .with_context(|| format!("downloading {} from {}", entry.file, entry.repo))?;
        fs::copy(&cached_path, &dest_path)?;
    }

    if let Some(expected) = entry.sha256 {
        let actual = sha256_file(&dest_path)?;
        if actual != expected {
            fs::remove_file(&dest_path).ok();
            return Err(anyhow!("checksum mismatch for {}: expected {expected}, got {actual}", entry.file));
        }
    } else {
        eprintln!("warning: no checksum on record for {} — skipping verification", entry.file);
    }

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_hashes_the_same() {
        let path = std::env::temp_dir().join("model_manager_test_a.txt");
        fs::write(&path, b"hello world").unwrap();
        assert_eq!(sha256_file(&path).unwrap(), sha256_file(&path).unwrap());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn finds_known_model() {
        assert!(find_model("whisper-base-en").is_some());
        assert!(find_model("does-not-exist").is_none());
    }
}

pub struct CleanupModelEntry {
    pub repo: &'static str,
    pub file: &'static str,
    pub size_mb: u64,
}

pub const CLEANUP_MODEL: CleanupModelEntry = CleanupModelEntry {
    repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
    file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    size_mb: 1100,
};

pub fn download_cleanup_model() -> Result<PathBuf> {
    let dest_dir = models_dir()?;
    let dest_path = dest_dir.join(CLEANUP_MODEL.file);
    if !dest_path.exists() {
        let api = hf_hub::api::sync::Api::new()?;
        let repo = api.model(CLEANUP_MODEL.repo.to_string());
        let cached_path = repo
            .get(CLEANUP_MODEL.file)
            .with_context(|| format!("downloading {}", CLEANUP_MODEL.file))?;
        fs::copy(&cached_path, &dest_path)?;
    }
    Ok(dest_path)
}