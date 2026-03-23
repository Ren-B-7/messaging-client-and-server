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

    info!("Config validated");

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

    // JWT secret must be resolvable (env var or config field) and long enough.
    // Validated here so a bad config is rejected immediately — including on
    // SIGHUP hot-reloads — rather than failing silently at the first login.
    match config.auth.resolved_jwt_secret() {
        None => {
            return Err(ConfigError::InvalidConfig(
                "jwt_secret must be set via the JWT_SECRET env var or auth.jwt_secret config field"
                    .into(),
            ));
        }
        Some(secret) if secret.len() < 32 => {
            return Err(ConfigError::InvalidConfig(
                "jwt_secret must be at least 32 characters long".into(),
            ));
        }
        _ => {}
    }

    Ok(())
}
