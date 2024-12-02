mod config;
mod processor;

use crate::processor::*;
use core_data::models::workflow::Workflow;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::config::config::load_config;
use mongodb::{Client, options::ClientOptions, bson::doc}; 
use futures::stream::TryStreamExt;

async fn load_workflows(mongo_uri: &str, db_name: &str, workflow_ids: &[String]) -> Result<Vec<Workflow>, mongodb::error::Error> {
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database(db_name);
    let collection = db.collection::<Workflow>("Workflow");

    let filter = doc! {
        "id": {
            "$in": workflow_ids
        }
    };

    let mut workflows = Vec::new();
    let mut cursor = collection.find(filter, None).await?;
    
    while let Some(workflow) = cursor.try_next().await? {
        workflows.push(workflow);
    }

    Ok(workflows)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config: {:?}", e);
            std::process::exit(1);
        }
    };
    println!("Config loaded: {:?}", config);

    let workflows = match load_workflows(&config.mongodburi, &config.mongodbdatabase, &config.workflowids).await {
        Ok(wf) => wf,
        Err(e) => {
            eprintln!("Failed to load workflows: {}", e);
            std::process::exit(1);
        }
    };
    println!("Loaded {} workflows", workflows.len());
    
    let processor = BatchProcessor::new(config, workflows)?;
    processor.run().await?;

    Ok(())
}