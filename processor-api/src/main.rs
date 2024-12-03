mod initiate;
mod config;

use actix_web::{web, App, HttpServer};
use tracing::{debug, error, info, instrument};
use crate::initiate::initiate_message;
use crate::config::config::load_config;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[instrument(name = "startup")]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let startup_span = tracing::info_span!("application_startup");
    let _guard = startup_span.entered();

    debug!("Initializing tracing system");
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| {
                std::env::var("RUST_LOG")
                    .unwrap_or_else(|_| "info,actix_web=info".to_string()).into()
            }))
        .with(tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
            .with_level(true))
        .init();

    let config = match load_config().await {
        Ok(cfg) => {
            info!(
                host = %cfg.serverhostname,
                port = %cfg.serverport,
                kafka_servers = %cfg.kafkabootstrapservers,
                "Configuration loaded successfully"
            );
            cfg
        }
        Err(e) => {
            error!(
                error = %e,
                "Failed to load configuration. Exiting application"
            );
            std::process::exit(1);
        }
    };

    let web_config = web::Data::new(config.clone());
    let bind_address = format!("{}:{}", &config.serverhostname, &config.serverport);
    
    info!(
        address = %bind_address,
        "Starting HTTP server"
    );
    
    HttpServer::new(move || {
        debug!("Initializing new worker");
        App::new()
            .app_data(web_config.clone())
            .service(web::resource("/initiate").to(initiate_message))
            .wrap(actix_web::middleware::Logger::default())
            .wrap(actix_web::middleware::Compress::default())
            .wrap(actix_web::middleware::DefaultHeaders::new()
                .add(("Access-Control-Allow-Origin", "*"))
                .add(("Access-Control-Allow-Methods", "POST"))
                .add(("Access-Control-Allow-Headers", "Content-Type")))
    })
    .bind(&bind_address)
    .map_err(|e| {
        error!(error = %e, address = %bind_address, "Failed to bind server");
        e
    })?
    .run()
    .await
}