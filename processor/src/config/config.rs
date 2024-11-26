use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub kafka_config: KafkaConfig,
    pub batch_config: BatchConfig,
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.kafka_config.input_topic.is_empty() || self.kafka_config.output_topic.is_empty() {
            return Err(ConfigError::ValidationError(
                "Input and output topics cannot be empty".to_string(),
            ));
        }
        if self.batch_config.batch_size == 0 {
            return Err(ConfigError::ValidationError(
                "Batch size must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub bootstrap_servers: String,
    pub group_id: String,
    pub input_topic: Vec<String>,
    pub output_topic: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
}

pub fn load_config() -> Result<Config, config::ConfigError> {
    config::Config::builder()
        .add_source(config::File::with_name("config"))
        .build()?
        .try_deserialize()
}