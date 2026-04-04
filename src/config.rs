use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/* Configuration loaded from ~/.config/zotero-cli/config.toml.
   All fields have sane defaults so the tool works out-of-the-box
   against a locally running Zotero instance. */

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_api_base")]
    pub api_base: String,

    pub api_key: Option<String>,

    pub user_id: Option<u64>,

    #[serde(default = "default_library_type")]
    pub library_type: String,
}

fn default_api_base() -> String {
    "http://localhost:23119/api".to_string()
}

fn default_library_type() -> String {
    "user".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            api_base: default_api_base(),
            api_key: None,
            user_id: None,
            library_type: default_library_type(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing config at {}", path.display()))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    pub fn path() -> PathBuf {
        config_path()
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("zotero-cli")
        .join("config.toml")
}
