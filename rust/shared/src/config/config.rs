use std::fs;
use tracing::{debug, error, info};

use crate::types::server_config::{AppConfig, ConfigError};

pub fn load_config(path: &str) -> Result<AppConfig, ConfigError> {
    info!("Loading configuration from: {}", path);

    let contents = fs::read_to_string(path)?;
    debug!("Processing file: {}", path);

    if contents.trim().is_empty() {
        error!("Configuration file is empty");
        return Err(ConfigError::InvalidConfig("empty file".into()));
    }

    let config: AppConfig = toml::from_str(&contents)?;

    info!("Configuration loaded successfully");
    debug!("Config: {:?}", config);

    validate_config(&config)?;

    Ok(config)
}

fn validate_config(config: &AppConfig) -> Result<(), ConfigError> {
    if config.paths.web_dir.is_empty() {
        return Err(ConfigError::InvalidConfig("web_dir cannot be empty".into()));
    }

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
