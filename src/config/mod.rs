// Author: kelexine (https://github.com/kelexine)
// config/mod.rs — Global configuration loader

use serde::Deserialize;

/// Represents the structure of the global `config.toml`.
#[derive(Deserialize, Default, Debug)]
pub struct GlobalConfig {
    pub warn_size: Option<usize>,
    pub default_types: Option<Vec<String>>,
    pub always_extract_functions: Option<bool>,
}

impl GlobalConfig {
    /// Attempt to load the global configuration, returning a default instance if it fails or missing.
    pub fn load() -> Self {
        if let Some(mut path) = dirs::config_dir() {
            path.push("loc-rs");
            path.push("config.toml");

            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    match toml::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => eprintln!("[WARNING] Failed to parse {}: {}", path.display(), e),
                    }
                }
            }
        }
        Self::default()
    }
}
