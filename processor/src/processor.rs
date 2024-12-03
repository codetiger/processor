use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
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

pub struct BatchProcessor {
    consumer: StreamConsumer,
    producer: FutureProducer,
    config: Arc<AppConfig>,
    workflows: Vec<Workflow>,
}

type ProcessResult<T> = Result<T, ProcessorError>;

impl BatchProcessor {
    #[instrument(skip(config, workflows), fields(group_id = %config.kafkagroupid))]
    pub fn new(config: AppConfig, workflows: Vec<Workflow>) -> ProcessResult<Self> {
        let consumer: StreamConsumer = Self::create_consumer(&config)?;
        let producer: FutureProducer = Self::create_producer(&config)?;

        let input_topics: Vec<&str> = workflows.iter()
            .map(|w| w.input_topic.as_str())
            .collect();
        
        consumer
            .subscribe(&input_topics)
            .map_err(ProcessorError::KafkaError)?;

        Ok(Self {
            consumer,
            producer,
            config: Arc::new(config),
            workflows,
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
        let mut batch = BatchBuffer::new(self.config.batchsize);
        let mut last_message_time = tokio::time::Instant::now();

        loop {
            tokio::select! {
                Some(message_result) = message_stream.next() => {
                    match message_result {
                        Ok(message) => {
                            batch.add_message(message.offset(), message);
                            last_message_time = tokio::time::Instant::now();

                            if batch.is_full() {
                                self.process_batch(&mut batch).await?;
                            }
                        }
                        Err(e) => {
                            error!("Error receiving message: {}", e);
                            continue;
                        }
                    }
                }
                _ = sleep(Duration::from_millis(self.config.batchtimeoutms)) => {
                    if batch.should_process(last_message_time, self.config.batchtimeoutms) {
                        self.process_batch(&mut batch).await?;
                    }
                }
            }
        }
    }

    async fn process_batch(&self, batch: &mut BatchBuffer<'_>) -> ProcessResult<()> {
        info!(
            batch_size = batch.len(),
            "Starting batch processing"
        );
        
        let start = std::time::Instant::now();
        let processing_results = self.process_messages(batch).await?;
        
        debug!(
            processing_time_ms = start.elapsed().as_millis(),
            messages_processed = processing_results.len(),
            "Batch processing completed"
        );
        
        self.produce_messages(batch, processing_results).await?;
        batch.clear();
        Ok(())
    }

    async fn process_messages(&self, batch: &BatchBuffer<'_>) -> ProcessResult<Vec<Vec<u8>>> {
        let processing_futures: Vec<_> = batch
            .messages()
            .enumerate()
            .map(|(idx, (offset, message))| async move {
                trace!(
                    index = idx,
                    offset = offset,
                    topic = message.topic(),
                    "Processing message"
                );
                let payload = message.payload().unwrap_or_default();
                process_message(payload, &self.workflows).await
            })
            .collect();
    
        let results = futures::future::join_all(processing_futures).await;
        
        let mut processed_messages = Vec::with_capacity(results.len());
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(processed) => {
                    trace!(
                        index = i,
                        size = processed.len(),
                        "Message processed successfully"
                    );
                    processed_messages.push(processed);
                }
                Err(e) => {
                    error!(
                        error = %e,
                        index = i,
                        "Failed to process message"
                    );
                    return Err(ProcessorError::ProcessingError(
                        format!("Batch processing failed at message {}: {}", i, e)
                    ));
                }
            }
        }
    
        Ok(processed_messages)
    }

    async fn produce_messages(
        &self,
        batch: &BatchBuffer<'_>,
        processed_messages: Vec<Vec<u8>>,
    ) -> ProcessResult<()> {
        if processed_messages.len() != batch.len() {
            error!(
                expected = batch.len(),
                actual = processed_messages.len(),
                "Message count mismatch"
            );
            return Err(ProcessorError::ProcessingError(
                "Processed messages count doesn't match batch size".to_string(),
            ));
        }
    
        for (idx, (processed_message, (offset, original_message))) 
            in processed_messages.into_iter().zip(batch.messages()).enumerate() {
            let headers = rdkafka::message::OwnedHeaders::new();
            
            match self.produce_single_message(original_message, processed_message, headers).await {
                Ok(_) => {
                    if let Err(e) = self.consumer.commit_message(original_message, CommitMode::Async) {
                        error!(
                            error = %e,
                            index = idx,
                            offset = offset,
                            topic = original_message.topic(),
                            "Failed to commit message"
                        );
                        return Err(ProcessorError::KafkaError(e));
                    }
                    debug!(
                        index = idx,
                        offset = offset,
                        topic = original_message.topic(),
                        "Message processed and committed"
                    );
                }
                Err(e) => {
                    error!(
                        error = %e,
                        index = idx,
                        offset = offset,
                        topic = original_message.topic(),
                        "Failed to produce message"
                    );
                    return Err(e);
                }
            }
        }
        Ok(())
    }
    
    async fn produce_single_message(
        &self,
        original_message: &rdkafka::message::BorrowedMessage<'_>,
        processed_message: Vec<u8>,
        headers: rdkafka::message::OwnedHeaders,
    ) -> ProcessResult<()> {
        trace!(
            message_size = processed_message.len(),
            "Producing processed message"
        );
        let topic = "output";
        match self.producer
        .send(
            FutureRecord::to(&topic)
                .payload(&processed_message)
                .key(original_message.key().unwrap_or_default())
                .headers(headers),
            Duration::from_secs(5),
        )
        .await {
            Ok(_) => Ok(()),
            Err((e, _msg)) => {
                error!(
                    error = %e,
                    offset = original_message.offset(),
                    partition = original_message.partition(),
                    topic = %original_message.topic(),
                    "Failed to produce message"
                );
                Err(ProcessorError::KafkaError(e))
            }
        }
    }
}

struct BatchBuffer<'a> {
    messages: Vec<(i64, rdkafka::message::BorrowedMessage<'a>)>,
    capacity: usize,
}

impl<'a> BatchBuffer<'a> {
    fn new(capacity: usize) -> Self {
        Self {
            messages: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn add_message(&mut self, offset: i64, message: rdkafka::message::BorrowedMessage<'a>) {
        self.messages.push((offset, message));
    }

    fn is_full(&self) -> bool {
        self.messages.len() >= self.capacity
    }

    fn should_process(&self, last_message_time: tokio::time::Instant, timeout_ms: u64) -> bool {
        !self.messages.is_empty() 
            && last_message_time.elapsed().as_millis() as u64 >= timeout_ms
    }

    fn messages(&self) -> impl Iterator<Item = &(i64, rdkafka::message::BorrowedMessage<'_>)> {
        self.messages.iter()
    }

    fn len(&self) -> usize {
        self.messages.len()
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}


#[instrument(skip(msg, workflows), fields(msg_size = msg.len()))]
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
    for workflow in workflows {
        if message.workflow_match(&workflow.tenant, &workflow.origin, &workflow.condition) {
            debug!(
                workflow_id = %workflow.id,
                message_id = %message.id(),
                "Executing workflow"
            );
            workflow_executed = true;
            message.execute_workflow(workflow)
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