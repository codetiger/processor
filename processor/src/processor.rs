use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::time::Duration;
use futures::StreamExt;
use tracing::{trace, debug, error, warn, info, instrument};
use serde_json::Error as JsonError;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use std::sync::Arc;

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

#[derive(Debug)]
struct MessageMetadata {
    topic: String,
    partition: i32,
    offset: i64,
    key: Vec<u8>,
}

pub struct Processor {
    consumer: Arc<StreamConsumer>,
    producer: FutureProducer,
    config: AppConfig,
    workflows: Arc<Vec<Workflow>>,
    semaphore: Arc<Semaphore>,
}

impl Processor {
    #[instrument(skip(config, workflows), fields(group_id = %config.kafkagroupid))]
    pub fn new(config: AppConfig, workflows: Vec<Workflow>) -> ProcessResult<Self> {
        let consumer = Arc::new(Self::create_consumer(&config)?);
        let producer = Self::create_producer(&config)?;
        let semaphore = Arc::new(Semaphore::new(config.maxconcurrency));

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
            workflows: Arc::new(workflows),
            semaphore,
        })
    }

    #[instrument(skip(config), fields(bootstrap_servers = %config.kafkabootstrapservers))]
    fn create_consumer(config: &AppConfig) -> ProcessResult<StreamConsumer> {
        ClientConfig::new()
            .set("group.id", &config.kafkagroupid)
            .set("bootstrap.servers", &config.kafkabootstrapservers)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()
            .map_err(ProcessorError::KafkaError)
    }

    #[instrument(skip(config), fields(bootstrap_servers = %config.kafkabootstrapservers))]
    fn create_producer(config: &AppConfig) -> ProcessResult<FutureProducer> {
        ClientConfig::new()
            .set("bootstrap.servers", &config.kafkabootstrapservers)
            .set("message.timeout.ms", "5000")
            .create()
            .map_err(ProcessorError::KafkaError)
    }

    #[instrument(skip(self), fields(max_concurrency = %self.config.maxconcurrency))]
    pub async fn run(&self) -> ProcessResult<()> {
        let mut message_stream = self.consumer.stream();
        let mut tasks: JoinSet<Result<(), ProcessorError>> = JoinSet::new();
    
        loop {
            if tasks.len() < self.config.maxconcurrency {
                tokio::select! {
                    Some(message_result) = message_stream.next() => {
                        match message_result {
                            Ok(message) => {
                                let permit = self.semaphore.clone().acquire_owned().await.unwrap();
                                let workflows = self.workflows.clone();
                                let producer = self.producer.clone();
                                let consumer = self.consumer.clone();
                                let payload = message.payload().unwrap_or_default().to_vec();
                                
                                let metadata = MessageMetadata {
                                    topic: message.topic().to_string(),
                                    partition: message.partition(),
                                    offset: message.offset(),
                                    key: message.key().unwrap_or_default().to_vec(),
                                };
    
                                tasks.spawn(async move {
                                    let _permit = permit;
                                    let processed = Self::process_message(&payload, &workflows).await?;
                                    
                                    let headers = rdkafka::message::OwnedHeaders::new();
                                    Self::publish_message(&producer, "message_updates", &metadata.key, processed, headers).await?;
                                    Self::commit_message(&consumer, &metadata).await?;

                                    Ok(())
                                });
                            }
                            Err(e) => error!("Error receiving message: {}", e),
                        }
                    }
                    Some(result) = tasks.join_next() => {
                        if let Err(e) = result {
                            error!("Task joined with error: {}", e);
                        }
                    }
                }
            } else {
                if let Some(result) = tasks.join_next().await {
                    if let Err(e) = result {
                        error!("Task joined with error: {}", e);
                    }
                }
            }
        }
    }


    #[instrument(skip(msg, workflows), fields(msg_size = msg.len(), workflow_count = workflows.len()))]
    async fn process_message(msg: &[u8], workflows: &[Workflow]) -> Result<Vec<u8>, ProcessorError> {
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
        for workflow in workflows.iter() {
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

    #[instrument(skip(producer, processed_message), fields(topic = %topic, message_size = %processed_message.len()))]
    async fn publish_message(producer: &FutureProducer, topic: &str, key: &[u8], processed_message: Vec<u8>, headers: rdkafka::message::OwnedHeaders,) -> ProcessResult<()> {
        trace!(
            message_size = processed_message.len(),
            "Producing processed message"
        );
        match producer.send(
                FutureRecord::to(&topic)
                    .payload(&processed_message)
                    .key(key)
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

    #[instrument(skip(consumer), fields(topic = %metadata.topic, partition = %metadata.partition, offset = %metadata.offset))]
    async fn commit_message(consumer: &StreamConsumer, metadata: &MessageMetadata) -> ProcessResult<()> {
        trace!(
            topic = %metadata.topic,
            partition = metadata.partition,
            offset = metadata.offset,
            "Committing message offset"
        );

        match consumer.store_offset(&metadata.topic, metadata.partition, metadata.offset) {
            Ok(_) => {
                info!(
                    topic = %metadata.topic,
                    partition = metadata.partition,
                    offset = metadata.offset,
                    "Offset committed successfully"
                );
                Ok(())
            },
            Err(e) => {
                error!(
                    error = %e,
                    topic = %metadata.topic,
                    partition = metadata.partition,
                    offset = metadata.offset,
                    "Failed to commit offset"
                );
                Err(ProcessorError::KafkaError(e))
            }
        }
    }
}

