use std::env;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct AppConfig {
    pub serverhostname: String,
    pub serverport: u16,
    pub kafkabootstrapservers: String,
    pub kafkamessagetimeoutms: String,
    pub kafkatopic: String,
}

#[derive(Debug)]
pub enum ConfigError {
    ParseError(String),
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let mut config = AppConfig::default();

    config.serverhostname = env::var("SERVERHOSTNAME")
        .unwrap_or_else(|_| String::from("127.0.0.1"));
    
    config.serverport = env::var("SERVERPORT")
        .unwrap_or_else(|_| String::from("8080"))
        .parse()
        .map_err(|e| ConfigError::ParseError(format!("Invalid port: {}", e)))?;
    
    config.kafkabootstrapservers = env::var("KAFKABOOTSTRAPSERVERS")
        .map_err(|e| ConfigError::ParseError(format!("Invalid KAFKABOOTSTRAPSERVERS: {}", e)))?;
    
    config.kafkamessagetimeoutms = env::var("KAFKAMESSAGETIMEOUTMS")
    .map_err(|e| ConfigError::ParseError(format!("Invalid KAFKAMESSAGETIMEOUTMS: {}", e)))?;
    
    config.kafkatopic = env::var("KAFKATOPIC")
    .map_err(|e| ConfigError::ParseError(format!("Invalid KAFKATOPIC: {}", e)))?;

    Ok(config)
}