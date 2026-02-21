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

            if path.exists()
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("[WARNING] Failed to parse {}: {}", path.display(), e),
                }
            }
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_global_config_full() {
        let toml_str = r#"
        warn_size = 500
        default_types = ["rust", "python"]
        always_extract_functions = true
        "#;
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.warn_size, Some(500));
        assert_eq!(
            config.default_types,
            Some(vec!["rust".to_string(), "python".to_string()])
        );
        assert_eq!(config.always_extract_functions, Some(true));
    }

    #[test]
    fn test_parse_global_config_empty() {
        let toml_str = "";
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.warn_size, None);
        assert_eq!(config.default_types, None);
        assert_eq!(config.always_extract_functions, None);
    }
}
