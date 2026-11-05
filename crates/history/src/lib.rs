//! Simple JSON-backed transcription history.
//!
//! Stores recent transcripts (and optionally a short note about the source)
//! under the OS data directory. Designed to be lightweight — no SQLite
//! dependency for the first version.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    /// "live" | "file" | "postprocessed"
    pub source: String,
    /// Optional path if this came from a file transcription.
    pub file_path: Option<String>,
    /// Whether the user starred / pinned this entry.
    pub starred: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
    /// Maximum number of non-starred entries to keep.
    max_entries: usize,
}

fn history_path() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow!("could not determine OS data directory"))?
        .join("localvox");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("history.json"))
}

fn load() -> HistoryFile {
    history_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| HistoryFile {
            entries: Vec::new(),
            max_entries: 200,
        })
}

fn save(file: &HistoryFile) -> Result<()> {
    let path = history_path()?;
    let json = serde_json::to_string_pretty(file)?;
    fs::write(path, json)?;
    Ok(())
}

/// Append a new transcript. Returns the created entry.
pub fn add(text: &str, source: &str, file_path: Option<String>) -> Result<HistoryEntry> {
    let mut file = load();
    let entry = HistoryEntry {
        id: Uuid::new_v4().to_string(),
        text: text.to_string(),
        created_at: Utc::now(),
        source: source.to_string(),
        file_path,
        starred: false,
    };
    file.entries.insert(0, entry.clone());

    // Prune non-starred entries beyond the limit.
    let mut kept = 0usize;
    file.entries.retain(|e| {
        if e.starred {
            return true;
        }
        if kept < file.max_entries {
            kept += 1;
            true
        } else {
            false
        }
    });

    save(&file)?;
    Ok(entry)
}

pub fn list(limit: usize) -> Vec<HistoryEntry> {
    let file = load();
    file.entries.into_iter().take(limit).collect()
}

pub fn star(id: &str, starred: bool) -> Result<()> {
    let mut file = load();
    if let Some(e) = file.entries.iter_mut().find(|e| e.id == id) {
        e.starred = starred;
        save(&file)?;
        Ok(())
    } else {
        Err(anyhow!("entry not found"))
    }
}

pub fn delete(id: &str) -> Result<()> {
    let mut file = load();
    let before = file.entries.len();
    file.entries.retain(|e| e.id != id);
    if file.entries.len() == before {
        return Err(anyhow!("entry not found"));
    }
    save(&file)?;
    Ok(())
}

pub fn clear_unstarred() -> Result<()> {
    let mut file = load();
    file.entries.retain(|e| e.starred);
    save(&file)?;
    Ok(())
}
