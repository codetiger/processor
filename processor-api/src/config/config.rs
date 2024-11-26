use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct KafkaConfig {
    pub bootstrap_servers: String,
    pub message_timeout_ms: String,
    pub topic: String
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerConfig {
    pub hostname: String,
    pub port: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub kafka: KafkaConfig,
}

pub fn load_config() -> Result<Config, config::ConfigError> {
    config::Config::builder()
        .add_source(config::File::with_name("config"))
        .build()?
        .try_deserialize()
}