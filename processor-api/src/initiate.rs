use core_data::models::message::*;
use serde::Serialize;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::json;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use tokio;
use tracing::{debug, error, info, instrument, warn};
use crate::config::config::*;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(untagged)] 
pub enum InitiationResponse {
    Success(serde_json::Value),
    Error(Vec<String>),
}

#[instrument(skip(message, config), fields(message_id = %message.id()))]
async fn publish_to_kafka(message: &Message, config: &AppConfig) -> Result<(), String> {
    let start = std::time::Instant::now();
    
    debug!(
        bootstrap_servers = %config.kafkabootstrapservers,
        topic = %config.kafkatopic,
        "Creating Kafka producer"
    );

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.kafkabootstrapservers)
        .set("message.timeout.ms", &config.kafkamessagetimeoutms)
        .create()
        .map_err(|e| {
            error!(error = %e, "Failed to create Kafka producer");
            format!("Producer creation error: {}", e)
        })?;

    let json_string = serde_json::to_string(&message)
        .map_err(|e| {
            error!(error = %e, "Failed to serialize message");
            format!("Message serialization error: {}", e)
        })?;

    debug!(
        message_size = json_string.len(),
        "Publishing message to Kafka"
    );

    producer
        .send(
            FutureRecord::to(&config.kafkatopic)
                .payload(json_string.as_bytes())
                .key(&message.id().to_string()),
            std::time::Duration::from_secs(5),
        )
        .await
        .map_err(|(e, _)| {
            error!(
                error = %e,
                topic = %config.kafkatopic,
                message_id = %message.id(),
                "Failed to deliver message to Kafka"
            );
            format!("Kafka delivery error: {}", e)
        })?;

    info!(
        duration_ms = start.elapsed().as_millis(),
        message_id = %message.id(),
        topic = %config.kafkatopic,
        "Successfully published message to Kafka"
    );

    Ok(())
}

#[instrument(skip(config, _req, body), fields(request_id = %Uuid::new_v4()))]
pub async fn initiate_message(
    config: web::Data<AppConfig>,
    _req: HttpRequest,
    body: String,
) -> impl Responder {
    debug!(body_size = body.len(), "Received initiation request");

    let initiation_result = tokio::spawn(async move {
        let payload = Payload::new_inline(
            Some(body.as_bytes().to_vec()),
            PayloadFormat::Xml,
            PayloadSchema::ISO20022,
            Encoding::Utf8,
        );
        
        let message = Message::new(
            payload,
            "tenant1".to_string(),
            "api".to_string(),
            "payment_processing".to_string(),
            1,
            "initiate".to_string(),
            Some("Payment".to_string()),
        );

        debug!(message_id = %message.id(), "Created new message");

        if let Err(e) = publish_to_kafka(&message, &config).await {
            error!(error = %e, "Message initiation failed");
            return InitiationResponse::Error(vec![format!("Kafka error: {}", e)]);
        }

        let response = json!({ "message_id": message.id().to_string() });
        info!(message_id = %message.id(), "Message initiated successfully");
        InitiationResponse::Success(response)
    })
    .await
    .unwrap_or_else(|e| {
        error!(error = %e, "Task execution failed");
        InitiationResponse::Error(vec![format!("Task error: {:?}", e)])
    });

    match &initiation_result {
        InitiationResponse::Success(data) => {
            info!(message_id = %data["message_id"], "Request completed successfully");
            HttpResponse::Ok().json(data)
        },
        InitiationResponse::Error(errors) => {
            warn!(errors = ?errors, "Request failed");
            HttpResponse::BadRequest().json(errors)
        },
    }
}