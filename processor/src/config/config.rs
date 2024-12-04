use std::env;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct AppConfig {
    pub kafkabootstrapservers: String,
    pub kafkagroupid: String,

    pub maxconcurrency: usize,

    pub mongodburi: String,
    pub mongodbdatabase: String,

    pub workflowids: Vec<String>,
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let mut config = AppConfig::default();

    config.kafkabootstrapservers = env::var("KAFKABOOTSTRAPSERVERS")?;
    config.kafkagroupid = env::var("KAFKAGROUPID")?;
    
    config.maxconcurrency = env::var("MAXCONCURRENCY")
        .unwrap_or_else(|_| String::from("1"))
        .parse()
        .map_err(|e| ConfigError::ParseError(format!("Invalid batch size: {}", e)))?;
    
    config.mongodburi = env::var("MONGODBURI")?;
    config.mongodbdatabase = env::var("MONGODBDATABASE")?;
    
    config.workflowids = env::var("WORKFLOWIDS")?
        .split(',')
        .map(String::from)
        .collect();

    Ok(config)
}