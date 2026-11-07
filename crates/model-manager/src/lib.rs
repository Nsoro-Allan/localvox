use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// One entry in the model catalog.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub size_mb: u64,
    /// "whisper" | "parakeet" | "canary" | "other"
    pub family: &'static str,
    /// Short description shown in the UI.
    pub description: &'static str,
    /// Languages (approximate).
    pub languages: &'static str,
    /// Recommended for most users?
    pub recommended: bool,
    /// Filled in once verified. None is fine (skip check).
    pub sha256: Option<&'static str>,
}

pub const MANIFEST: &[ModelEntry] = &[
    // ---------- Whisper (fully supported) ----------
    ModelEntry {
        id: "whisper-tiny-en",
        display_name: "Whisper Tiny (EN)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-tiny.en.bin",
        size_mb: 75,
        family: "whisper",
        description: "Fastest English-only. Good for low-end CPUs.",
        languages: "English",
        recommended: false,
        sha256: None,
    },
    ModelEntry {
        id: "whisper-base-en",
        display_name: "Whisper Base (EN)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-base.en.bin",
        size_mb: 148,
        family: "whisper",
        description: "Good speed/accuracy balance for English.",
        languages: "English",
        recommended: false,
        sha256: None,
    },
    ModelEntry {
        id: "whisper-small-en",
        display_name: "Whisper Small (EN)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-small.en.bin",
        size_mb: 488,
        family: "whisper",
        description: "Solid English accuracy on mid-range machines.",
        languages: "English",
        recommended: false,
        sha256: None,
    },
    ModelEntry {
        id: "whisper-medium-en",
        display_name: "Whisper Medium (EN)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-medium.en.bin",
        size_mb: 1500,
        family: "whisper",
        description: "High English accuracy. Needs more RAM.",
        languages: "English",
        recommended: false,
        sha256: None,
    },
    ModelEntry {
        id: "whisper-large-v3-turbo",
        display_name: "Whisper Large-v3 Turbo",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-large-v3-turbo.bin",
        size_mb: 1600,
        family: "whisper",
        description: "Multilingual, high quality, faster than full Large.",
        languages: "99+ languages",
        recommended: true,
        sha256: None,
    },
    ModelEntry {
        id: "whisper-small",
        display_name: "Whisper Small (multilingual)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-small.bin",
        size_mb: 487,
        family: "whisper",
        description: "Multilingual Small. Good general-purpose pick.",
        languages: "99+ languages",
        recommended: true,
        sha256: None,
    },
    ModelEntry {
        id: "whisper-medium",
        display_name: "Whisper Medium (multilingual)",
        repo: "ggerganov/whisper.cpp",
        file: "ggml-medium.bin",
        size_mb: 1500,
        family: "whisper",
        description: "Strong multilingual accuracy.",
        languages: "99+ languages",
        recommended: false,
        sha256: None,
    },
];

pub fn find_model(id: &str) -> Option<&'static ModelEntry> {
    MANIFEST.iter().find(|m| m.id == id)
}

pub fn models_by_family(family: &str) -> Vec<&'static ModelEntry> {
    MANIFEST.iter().filter(|m| m.family == family).collect()
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
        if n == 0 {
            break;
        }
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
        let cached_path = repo.get(entry.file).with_context(|| {
            format!(
                "downloading {} from {}. If this repo/file does not exist yet, \
                 the model is catalogued for future support — use a Whisper model for now.",
                entry.file, entry.repo
            )
        })?;
        fs::copy(&cached_path, &dest_path)?;
    }

    if let Some(expected) = entry.sha256 {
        let actual = sha256_file(&dest_path)?;
        if actual != expected {
            fs::remove_file(&dest_path).ok();
            return Err(anyhow!(
                "checksum mismatch for {}: expected {expected}, got {actual}",
                entry.file
            ));
        }
    }

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_known_model() {
        assert!(find_model("whisper-base-en").is_some());
        assert!(find_model("whisper-large-v3-turbo").is_some());
        assert!(find_model("does-not-exist").is_none());
    }

    #[test]
    fn recommended_includes_turbo_and_small() {
        let rec: Vec<_> = MANIFEST.iter().filter(|m| m.recommended).map(|m| m.id).collect();
        assert!(rec.contains(&"whisper-large-v3-turbo"));
        assert!(rec.contains(&"whisper-small"));
    }
}