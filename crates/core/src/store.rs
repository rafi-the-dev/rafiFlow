use crate::Mode;
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub custom: String,
    pub vocabulary: String,
    pub asr_model: String,
    pub crispasr: String,
    pub gguf: String,
    pub llama_server: String,
    pub gpu_layers: u32,
    pub auto_paste: bool,
    pub keep_history: bool,
}
impl Default for Settings {
    fn default() -> Self {
        let base = directories::ProjectDirs::from("", "", "rafiflow")
            .map(|dirs| dirs.data_local_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from(".rafiflow"));
        let base = base.to_string_lossy();
        Self {
            mode: Mode::Aggressive,
            custom: String::new(),
            vocabulary: "rafiFlow, Hyprland, Omarchy, AGENTS.md, Rust, Tauri, Yew".into(),
            asr_model: format!("{base}/models/cohere-transcribe-q8_0.gguf"),
            crispasr: format!("{base}/CrispASR/build/bin/crispasr"),
            gguf: format!("{base}/models/qwen3-4b-instruct-2507-q4_k_m.gguf"),
            llama_server: "llama-server".into(),
            gpu_layers: 24,
            auto_paste: true,
            keep_history: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub timestamp: u64,
    pub raw: String,
    pub text: String,
    pub mode: Mode,
    pub error: Option<String>,
}
pub fn data_dir() -> Result<PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "rafiflow")
        .context("Cannot determine application data directory")?
        .data_local_dir()
        .to_owned();
    private_dir(&dir)?;
    Ok(dir)
}
pub fn private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(
        &fs::read(path).with_context(|| format!("Read {}", path.display()))?,
    )?)
}
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temp = path.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    fs::rename(temp, path)?;
    Ok(())
}
pub fn history(dir: &Path) -> Result<Vec<Entry>> {
    let path = dir.join("history.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    read(&path)
}
pub fn append(dir: &Path, entry: Entry) -> Result<()> {
    let mut entries = history(dir)?;
    if let Some(index) = entries.iter().position(|existing| existing.id == entry.id) {
        entries[index] = entry;
    } else {
        entries.insert(0, entry);
    }
    entries.truncate(500);
    save(&dir.join("history.json"), &entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_preserves_raw_and_corruption_is_not_overwritten() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        private_dir(&dir).unwrap();
        let entry = Entry {
            id: "1".into(),
            timestamp: 0,
            raw: "actually no".into(),
            text: "No.".into(),
            mode: Mode::Aggressive,
            error: None,
        };
        append(&dir, entry.clone()).unwrap();
        assert_eq!(history(&dir).unwrap()[0].raw, entry.raw);
        fs::write(dir.join("history.json"), b"broken").unwrap();
        assert!(append(&dir, entry).is_err());
        assert_eq!(fs::read(dir.join("history.json")).unwrap(), b"broken");
        fs::remove_dir_all(dir).unwrap();
    }
}
