use serde::Deserialize;
use std::fs;
use thiserror::Error;
use tracing::{debug, error};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub assets: AssetConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub max_connections: usize,
}

#[derive(Debug, Deserialize)]
pub struct AssetConfig {
    pub mode: String,
    pub web_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub token_expiry_minutes: u64,
}

pub fn load_config(path: &str) -> Result<Config, ConfigError> {
    let contents = fs::read_to_string(path)?;
    debug!("Processing file: {}", path);

    if contents.trim().is_empty() {
        error!("File is empty");
        return Err(ConfigError::InvalidConfig("empty file".into()));
    }

    let config: Config = toml::from_str(&contents).map_err(|e| {
        error!("TOML parse error: {}", e);
        ConfigError::InvalidConfig(format!("TOML parse error: {}", e))
    })?;

    Ok(config)
}
