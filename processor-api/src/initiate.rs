use core_data::models::message::*;

use serde::Serialize;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::json;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use tokio;
use crate::config::config::*;


#[derive(Serialize)]
#[serde(untagged)] 
pub enum InitiationResponse {
    Success(serde_json::Value),
    Error(Vec<String>),
}

async fn publish_to_kafka(message: &Message, config: &AppConfig) -> Result<(), String> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.kafkabootstrapservers)
        .set("message.timeout.ms", &config.kafkamessagetimeoutms)
        .create()
        .map_err(|e| format!("Producer creation error: {}", e))?;

        let json_string = serde_json::to_string(&message)
        .map_err(|e| format!("Message serialization error: {}", e))?;

    producer
        .send(
            FutureRecord::to(&config.kafkatopic)
                .payload(json_string.as_bytes())
                .key(&message.id().to_string()),
            std::time::Duration::from_secs(5),
        )
        .await
        .map_err(|(e, _)| format!("Kafka delivery error: {}", e))?;

    Ok(())
}

pub async fn initiate_message(
    config: web::Data<AppConfig>,
    _req: HttpRequest,
    body: String,
) -> impl Responder {
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
            "processor".to_string(),
            1,
            "initiate".to_string(),
            Some("Payment".to_string()),
        );

        if let Err(e) = publish_to_kafka(&message, &config).await {
            return InitiationResponse::Error(vec![format!("Kafka error: {}", e)]);
        }

        let response = json!({ "message_id": message.id().to_string() });
        InitiationResponse::Success(response)
    })
    .await
    .unwrap_or_else(|e| InitiationResponse::Error(
        vec![format!("Task error: {:?}", e)]
    ));

    match initiation_result {
        InitiationResponse::Success(data) => HttpResponse::Ok().json(data),
        InitiationResponse::Error(errors) => HttpResponse::BadRequest().json(errors),
    }
}