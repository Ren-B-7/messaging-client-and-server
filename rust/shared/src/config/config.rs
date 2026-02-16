use serde::Deserialize;
use std::{collections::HashSet, fs};
use thiserror::Error;
use tracing::{debug, error, info};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub bind: String,
    #[serde(default = "default_admin_port")]
    pub port_admin: Option<u16>,
    #[serde(default = "default_client_port")]
    pub port_client: Option<u16>,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Paths {
    pub icons: String,
    pub web_dir: String,
    #[serde(default)]
    pub blocked_paths: HashSet<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    #[serde(default = "default_token_expiry")]
    pub token_expiry_minutes: u64,
    #[serde(default)]
    pub email_required: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub paths: Paths,
    pub auth: AuthConfig,
}

// Default value functions
fn default_admin_port() -> Option<u16> {
    Some(1338)
}

fn default_client_port() -> Option<u16> {
    Some(1337)
}

fn default_max_connections() -> usize {
    1000
}

fn default_token_expiry() -> u64 {
    60
}

pub fn load_config(path: &str) -> Result<Config, ConfigError> {
    info!("Loading configuration from: {}", path);

    let contents = fs::read_to_string(path)?;
    debug!("Processing file: {}", path);

    if contents.trim().is_empty() {
        error!("Configuration file is empty");
        return Err(ConfigError::InvalidConfig("empty file".into()));
    }

    let config: Config = toml::from_str(&contents)?;

    info!("Configuration loaded successfully");
    debug!("Config: {:?}", config);

    // Validate config
    validate_config(&config)?;

    Ok(config)
}

fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Validate web_dir exists or can be created
    if config.paths.web_dir.is_empty() {
        return Err(ConfigError::InvalidConfig("web_dir cannot be empty".into()));
    }

    // Validate token expiry is reasonable
    if config.auth.token_expiry_minutes == 0 {
        return Err(ConfigError::InvalidConfig(
            "token_expiry_minutes must be greater than 0".into(),
        ));
    }

    if config.server.max_connections == 0 {
        return Err(ConfigError::InvalidConfig(
            "max_connections must be greater than 0".into(),
        ));
    }

    Ok(())
}
