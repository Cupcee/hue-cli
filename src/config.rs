use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub bridge_ip: Option<String>,
    pub username: Option<String>,
    #[serde(default)]
    pub presets: HashMap<String, Preset>,
}

/// A named preset: one or more group actions applied together.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Preset {
    pub actions: Vec<PresetAction>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PresetAction {
    pub group: String,
    /// Brightness 0–100 (%)
    pub dim: Option<u8>,
    /// RGB values 0–255
    pub rgb: Option<[u8; 3]>,
}

impl Config {
    fn config_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        Ok(home.join(".config").join("hue-cli").join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn require_bridge_ip(&self) -> Result<&str> {
        self.bridge_ip
            .as_deref()
            .ok_or_else(|| anyhow!("Bridge not configured — run `hue init` first"))
    }

    pub fn require_username(&self) -> Result<&str> {
        self.username
            .as_deref()
            .ok_or_else(|| anyhow!("Not authenticated — run `hue init` first"))
    }
}
