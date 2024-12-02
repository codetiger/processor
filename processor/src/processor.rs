use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use futures::StreamExt;
use tracing::{debug, error, info};

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
}

pub struct BatchProcessor {
    consumer: StreamConsumer,
    producer: FutureProducer,
    config: Arc<AppConfig>,
    workflows: Vec<Workflow>,
}

type ProcessResult<T> = Result<T, ProcessorError>;

impl BatchProcessor {
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
        debug!("Processing batch of {} messages", batch.len());
        
        let processing_results = self.process_messages(batch).await?;
        self.produce_messages(batch, processing_results).await?;
        
        batch.clear();
        Ok(())
    }

    async fn process_messages(&self, batch: &BatchBuffer<'_>) -> ProcessResult<Vec<Vec<u8>>> {
        let processing_futures: Vec<_> = batch
            .messages()
            .map(|(_, message)| async {
                let payload = message.payload().unwrap_or_default();
                process_message(payload).await
            })
            .collect();

        let results = futures::future::join_all(processing_futures).await;
        
        // Collect successful results and log errors
        let mut processed_messages = Vec::with_capacity(results.len());
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(processed) => processed_messages.push(processed),
                Err(e) => {
                    error!("Failed to process message at index {}: {}", i, e);
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
            return Err(ProcessorError::ProcessingError(
                "Processed messages count doesn't match batch size".to_string(),
            ));
        }

        for (processed_message, (_, original_message)) in processed_messages.into_iter().zip(batch.messages()) {
            let headers = rdkafka::message::OwnedHeaders::new();
            
            match self.produce_single_message(original_message, processed_message, headers).await {
                Ok(_) => {
                    self.commit_message(original_message).await?;
                    debug!("Message at offset {} processed", original_message.offset());
                }
                Err(e) => {
                    error!("Failed to produce message: {}", e);
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
        // self.producer
        //     .send(
        //         FutureRecord::to(&self.config.kafka.output_topic[0])
        //             .payload(&processed_message)
        //             .key(original_message.key().unwrap_or_default())
        //             .headers(headers),
        //         Duration::from_secs(5),
        //     )
        //     .await
        //     .map_err(|(e, _)| ProcessorError::KafkaError(e))?;
        Ok(())
    }

    async fn commit_message(&self, message: &rdkafka::message::BorrowedMessage<'_>) -> ProcessResult<()> {
        self.consumer
            .commit_message(message, CommitMode::Async)
            .map_err(ProcessorError::KafkaError)
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


async fn process_message(msg: &[u8]) -> Result<Vec<u8>, ProcessorError> {
    // Validate message
    if msg.is_empty() {
        return Err(ProcessorError::ProcessingError(
            "Cannot process empty message".to_string(),
        ));
    }

    let message: core_data::models::message::Message = serde_json::from_slice(msg)
        .map_err(|e| ProcessorError::ProcessingError(format!("Error deserializing message: {}", e)))?;
    
    let json_string = serde_json::to_string(&message).unwrap();

    Ok(json_string.as_bytes().to_vec())
}
