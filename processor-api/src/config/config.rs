use std::env;
use std::fmt;
use std::time::Instant;
use tracing::{debug, error, info, info_span, instrument};

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
    EnvVarError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ParseError(msg) => write!(f, "Configuration parse error: {}", msg),
            ConfigError::EnvVarError(msg) => write!(f, "Environment variable error: {}", msg),
        }
    }
}

#[instrument(name = "config_loading", err(Display))]
pub async fn load_config() -> Result<AppConfig, ConfigError> {
    let start = Instant::now();
    let config_span = info_span!("configuration");
    let _guard = config_span.entered();

    info!("Starting configuration load");
    let mut config = AppConfig::default();

    // Load server configuration
    {
        let _server_span = info_span!("server_config").entered();
        config.serverhostname = env::var("SERVERHOSTNAME")
            .unwrap_or_else(|_| {
                debug!(default = "127.0.0.1", "Using default server hostname");
                String::from("127.0.0.1")
            });
        
        config.serverport = match env::var("SERVERPORT")
            .unwrap_or_else(|_| {
                debug!(default = "8080", "Using default server port");
                String::from("8080")
            })
            .parse() {
                Ok(port) => {
                    debug!(port = port, "Server port configured");
                    port
                },
                Err(e) => {
                    error!(
                        error = %e,
                        env_var = "SERVERPORT",
                        attempted_value = env::var("SERVERPORT").unwrap_or_default(),
                        "Invalid server port configuration"
                    );
                    return Err(ConfigError::ParseError(format!("Invalid port: {}", e)));
                }
            };
    }

    // Load Kafka configuration
    {
        let _kafka_span = info_span!("kafka_config").entered();
        let kafka_vars = [
            ("KAFKABOOTSTRAPSERVERS", &mut config.kafkabootstrapservers),
            ("KAFKAMESSAGETIMEOUTMS", &mut config.kafkamessagetimeoutms),
            ("KAFKATOPIC", &mut config.kafkatopic),
        ];

        for (var_name, config_field) in kafka_vars {
            *config_field = env::var(var_name).map_err(|e| {
                error!(
                    error = %e,
                    env_var = var_name,
                    "Missing required environment variable"
                );
                ConfigError::EnvVarError(format!("Missing {}: {}", var_name, e))
            })?;
            debug!(
                env_var = var_name,
                value = %config_field,
                "Loaded Kafka configuration"
            );
        }
    }

    info!(
        duration_ms = start.elapsed().as_millis(),
        host = %config.serverhostname,
        port = %config.serverport,
        kafka_servers = %config.kafkabootstrapservers,
        kafka_topic = %config.kafkatopic,
        "Configuration loaded successfully"
    );
    
    Ok(config)
}