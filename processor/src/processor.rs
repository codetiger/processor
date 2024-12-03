use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use rdkafka::message::BorrowedMessage;
use std::time::Duration;
use futures::StreamExt;
use tracing::{trace, debug, error, warn, info, instrument};
use serde_json::Error as JsonError;

use crate::config::config::*;
use core_data::models::workflow::Workflow;

#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    #[error("Kafka error: {0}")]
    KafkaError(#[from] KafkaError),
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
    #[error("Processing error: {0}")]
    ProcessingError(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] JsonError),
}

type ProcessResult<T> = Result<T, ProcessorError>;

pub struct Processor {
    consumer: StreamConsumer,
    producer: FutureProducer,
    config: AppConfig,
    workflows: Vec<Workflow>,
}

impl Processor {
    #[instrument(skip(config, workflows), fields(group_id = %config.kafkagroupid))]
    pub fn new(config: AppConfig, workflows: Vec<Workflow>) -> ProcessResult<Self> {
        let consumer = Self::create_consumer(&config)?;
        let producer = Self::create_producer(&config)?;

        let input_topics: Vec<&str> = workflows.iter()
            .map(|w| w.input_topic.as_str())
            .collect();
        
        consumer
            .subscribe(&input_topics)
            .map_err(ProcessorError::KafkaError)?;

            Ok(Self {
                consumer,
                producer,
                config,
                workflows
            })
        }

    fn create_consumer(config: &AppConfig) -> ProcessResult<StreamConsumer> {
        ClientConfig::new()
            .set("group.id", &config.kafkagroupid)
            .set("bootstrap.servers", &config.kafkabootstrapservers)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()
            .map_err(ProcessorError::KafkaError)
    }

    fn create_producer(config: &AppConfig) -> ProcessResult<FutureProducer> {
        ClientConfig::new()
            .set("bootstrap.servers", &config.kafkabootstrapservers)
            .set("message.timeout.ms", "5000")
            .create()
            .map_err(ProcessorError::KafkaError)
    }

    pub async fn run(&self) -> ProcessResult<()> {
        info!("Starting batch processor");
        let mut message_stream = self.consumer.stream();

        loop {
            tokio::select! {
                Some(message_result) = message_stream.next() => {
                    match message_result {
                        Ok(message) => {
                            let payload = message.payload().unwrap_or_default();
                            match self.process_message(payload).await {
                                Ok(processed) => {
                                    let headers = rdkafka::message::OwnedHeaders::new();
                                    self.publish_message("message_updates", &message, processed, headers).await?;
                                    self.consumer.commit_message(&message, CommitMode::Async)?;
                                }
                                Err(e) => {
                                    error!(
                                        error = %e,
                                        offset = message.offset(),
                                        partition = message.partition(),
                                        topic = message.topic(),
                                        "Failed to process message"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error receiving message: {}", e);
                            continue;
                        }
                    }
                }
            }
        }
    }

    #[instrument(skip(self, msg), fields(msg_size = msg.len()))]
    pub async fn process_message(&self, msg: &[u8]) -> Result<Vec<u8>, ProcessorError> {
        let start = std::time::Instant::now();

        if msg.is_empty() {
            error!("Received empty message");
            return Err(ProcessorError::ProcessingError(
                "Cannot process empty message".to_string(),
            ));
        }

        let mut message: core_data::models::message::Message = serde_json::from_slice(msg)
            .map_err(|e| {
                error!(error = %e, "Failed to deserialize message");
                ProcessorError::ProcessingError(format!("Error deserializing message: {}", e))
            })?;
        
        debug!(
            message_id = %message.id(),
            tenant = %message.tenant(),
            "Processing message"
        );

        let mut workflow_executed = false;
        for workflow in self.workflows.iter() {
            if message.workflow_match(&workflow.tenant, &workflow.origin, &workflow.condition) {
                debug!(
                    workflow_id = %workflow.id,
                    message_id = %message.id(),
                    "Executing workflow"
                );
                workflow_executed = true;
                message.execute_workflow(&workflow)
                    .map_err(|e| {
                        error!(
                            error = %e,
                            workflow_id = %workflow.id,
                            message_id = %message.id(),
                            "Workflow execution failed"
                        );
                        ProcessorError::ProcessingError(format!("Workflow execution error: {}", e))
                    })?;
                break;
            }
        }

        if !workflow_executed {
            warn!(
                message_id = %message.id(),
                tenant = %message.tenant(),
                "No matching workflow found"
            );
        }

        info!(
            duration_ms = start.elapsed().as_millis(),
            message_id = %message.id(),
            "Message processing completed"
        );

        serde_json::to_vec(&message).map_err(|e| {
            error!(error = %e, "Failed to serialize processed message");
            ProcessorError::SerializationError(e)
        })
    }

    pub async fn publish_message(&self, topic: &str, original_message: &BorrowedMessage<'_>, processed_message: Vec<u8>, headers: rdkafka::message::OwnedHeaders,) -> ProcessResult<()> {
        trace!(
            message_size = processed_message.len(),
            "Producing processed message"
        );
        match self.producer.send(
                FutureRecord::to(&topic)
                    .payload(&processed_message)
                    .key(original_message.key().unwrap_or_default())
                    .headers(headers),
                Duration::from_secs(5),
            )
        .await {
            Ok(_) => {
                info!(
                    topic = topic,
                    "Message produced sucessfully"
                );
                Ok(())
            },
            Err((e, _msg)) => {
                error!(
                    error = %e,
                    topic = topic,
                    "Failed to produce message"
                );
                Err(ProcessorError::KafkaError(e))
            }
        }
    }
}