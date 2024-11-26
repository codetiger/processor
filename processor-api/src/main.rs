mod initiate;
mod config;

use actix_web::{web, App, HttpServer};
use crate::initiate::initiate_message;
use crate::config::config::load_config;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load config at startup
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    println!("Starting server at http://{}:{}", &config.server.hostname, &config.server.port);
    let web_config = web::Data::new(config.clone());
    
    HttpServer::new(move || {
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
    .bind(format!("{}:{}", &config.server.hostname, &config.server.port))?
    .run()
    .await
}