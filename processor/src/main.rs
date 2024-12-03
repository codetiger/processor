mod config;
mod processor;

use crate::processor::*;
use core_data::models::workflow::Workflow;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::config::config::load_config;
use mongodb::{Client, options::ClientOptions, bson::doc}; 
use futures::stream::TryStreamExt;
use tracing::{debug, error, info, trace, info_span, instrument};

#[instrument(name = "load_workflows", skip(mongo_uri), fields(db = %db_name))]
async fn load_workflows(mongo_uri: &str, db_name: &str, workflow_ids: &[String]) -> Result<Vec<Workflow>, mongodb::error::Error> {
    let start = std::time::Instant::now();
    debug!(workflow_count = workflow_ids.len(), "Loading workflows from MongoDB");

    let client_options = ClientOptions::parse(mongo_uri).await
        .map_err(|e| {
            error!(error = %e, "Failed to parse MongoDB connection options");
            e
        })?;

    let client = Client::with_options(client_options)
        .map_err(|e| {
            error!(error = %e, "Failed to create MongoDB client");
            e
        })?;

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
        trace!(workflow_id = %workflow.id, "Loaded workflow");
        workflows.push(workflow);
    }

    info!(
        duration_ms = start.elapsed().as_millis(),
        workflow_count = workflows.len(),
        "Successfully loaded workflows"
    );

    Ok(workflows)
}


#[tokio::main]
#[instrument(name = "main")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let startup_span = info_span!("startup");
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

    let config = match load_config() {
        Ok(cfg) => {
            info!("Configuration loaded successfully: {:#?}", cfg);
            cfg
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    let workflows = match load_workflows(&config.mongodburi, &config.mongodbdatabase, &config.workflowids).await {
        Ok(wf) => {
            info!(
                workflow_count = wf.len(),
                database = %config.mongodbdatabase,
                "Successfully loaded workflows"
            );
            wf
        }
        Err(e) => {
            error!(
                error = %e,
                database = %config.mongodbdatabase,
                "Failed to load workflows"
            );
            std::process::exit(1);
        }
    };
    
    let processor = BatchProcessor::new(config, workflows)?;
    processor.run().await?;

    Ok(())
}