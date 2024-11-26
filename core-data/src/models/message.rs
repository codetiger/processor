use std::fs::File;
use std::io::{BufReader, BufRead};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::OffsetDateTime;
use sonyflake::Sonyflake;
use quick_xml::de::from_reader;

use datalogic_rs::JsonLogic;

use crate::models::payload::*;
use crate::models::auditlog::*;
use crate::models::errors::FunctionResponseError;
use crate::models::iso20022::ISO20022Message;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Progress {
    pub status: MessageStatus,

    pub workflow_id: String,

    pub prev_task: String,
    
    pub prev_status_code: Option<StatusCode>,
    
    #[serde(with = "time::serde::iso8601")]
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MessageStatus {
    Recieved,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EnrichmentConfig {
    pub field: String,

    pub rule: Value,

    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum StatusCode {
    Success,
    Failure,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Message {
    #[serde(rename = "id")]
    id: u64,

    parent_id: Option<String>,

    payload: Payload,
    
    tenant: String,
    
    origin: String,
    
    data: Value,

    metadata: Value,
    
    progress: Progress,

    audit: Vec<AuditLog>,

    #[serde(skip)]
    transaction_changes: Option<Vec<(String, Value)>>,
}

impl Message {
    pub fn audit(&self) -> &Vec<AuditLog> {
        &self.audit
    }

    pub fn data(&self) -> &Value {
        &self.data
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn metadata(&self) -> &Value {
        &self.metadata
    }

    pub fn origin(&self) -> &String {
        &self.origin
    }

    pub fn parent_id(&self) -> &Option<String> {
        &self.parent_id
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn tenant(&self) -> &String {
        &self.tenant
    }

    fn transaction_begin(&mut self, workflow: String, task: String) {
        self.progress.workflow_id = workflow;
        self.progress.prev_task = task;
        self.transaction_changes = Some(Vec::new());
    }

    fn transaction_rollback(&mut self) {
        if let Some(changes) = self.transaction_changes.take() {
            for (field_path, old_value) in changes.iter().rev() {
                let parts: Vec<&str> = field_path.split('.').collect();
                let mut current = &mut self.data;
                
                // Navigate to the parent object
                for part in parts.iter().take(parts.len() - 1) {
                    if !current[part].is_object() {
                        break;
                    }
                    current = current.get_mut(part).unwrap();
                }
                
                // Restore the old value
                if let Some(last_part) = parts.last() {
                    current[*last_part] = old_value.clone();
                }
            }
        }
        self.progress.prev_status_code = Some(StatusCode::Failure);
        self.progress.timestamp = OffsetDateTime::now_utc();
    }

    fn transaction_commit(&mut self) {
        self.progress.prev_status_code = Some(StatusCode::Success);
        self.progress.timestamp = OffsetDateTime::now_utc();
        self.transaction_changes = None;
    }

    fn update(&mut self, field_path: &str, new_value: Value) -> Result<(), FunctionResponseError> {
        let parts: Vec<&str> = field_path.split('.').collect();
        
        if parts[0] != "data" {
            return Err(FunctionResponseError::new(
                "Update".to_string(),
                400,
                "Invalid field path".to_string()
            ));
        }

        let mut current = &mut self.data;
        for (i, part) in parts.iter().enumerate().skip(1) {
            if i == parts.len() - 1 {
                // Store old value for potential rollback
                if let Some(changes) = &mut self.transaction_changes {
                    changes.push((field_path.to_string(), current[part].clone()));
                }
                current[part] = new_value.clone();
            } else {
                if !current[part].is_object() {
                    current[part] = json!({});
                }
                current = current.get_mut(part).unwrap();
            }
        }
        Ok(())
    }

    pub fn new(payload: Payload, tenant: String, origin: String, workflow: String, task: String, message_alias: Option<String>) -> Self {
        let start_time = OffsetDateTime::now_utc();
        let sf = Sonyflake::new().unwrap();
        let id = sf.next_id().unwrap();

        let alias = message_alias.unwrap_or_else(|| "Message".to_string());
        let description = alias
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + &alias[1..])
            .unwrap_or_else(|| "Message".to_string()) + " created";
        let reason = "Initial message creation for ".to_string() + &alias.to_lowercase();

        let change_log = ChangeLog::new("payload".to_string(), reason, None, None);
        let audit = AuditLog::new(
            workflow.to_string(), 
            task.to_string(), 
            start_time,
            description.to_string(),
            vec![change_log]
        );

        Self {
            id,
            parent_id: None,
            payload,
            tenant,
            origin,
            data: Value::Null,
            metadata: Value::Null,
            progress: Progress {
                status: MessageStatus::Recieved,
                workflow_id: workflow.to_string(),
                prev_task: task.to_string(),
                prev_status_code: Some(StatusCode::Success),
                timestamp: OffsetDateTime::now_utc(),
            },
            audit: vec![audit],
            transaction_changes: Some(Vec::new()),
        }
    }
    
    pub fn enrich(&mut self, config: Vec<EnrichmentConfig>, data: serde_json::Value, description: Option<String>, workflow: String, task: String) -> Result<(), FunctionResponseError> {
        let start_time = OffsetDateTime::now_utc();
        let logic = JsonLogic::new();
        let mut changes = Vec::new();
        
        // Begin transaction
        self.transaction_begin(workflow.clone(), task.clone());

        for cfg in config {
            let value = match logic.apply(&cfg.rule, &data) {
                Ok(v) => v,
                Err(e) => {
                    // Rollback on error
                    self.transaction_rollback();
                    return Err(FunctionResponseError::new(
                        "Enrichment".to_string(),
                        400,
                        format!("Rule application failed: {:?}", e)
                    ));
                }
            };

            // Update with transaction support
            if let Err(e) = self.update(&cfg.field, value.clone()) {
                self.transaction_rollback();
                return Err(e);
            }

            // Record change for audit
            changes.push(ChangeLog::new(
                cfg.field.to_string(),
                cfg.description.unwrap_or_else(|| format!("Enriched field {}", cfg.field)),
                None,
                Some(value)
            ));
        }

        // Commit transaction
        self.transaction_commit();

        // Create audit log
        let audit_log = AuditLog::new(
            workflow.to_string(),
            task.to_string(),
            start_time,
            description.unwrap_or_else(|| "Enrichment applied".to_string()),
            changes
        );
        self.audit.push(audit_log);
        Ok(())
    }

    pub fn parse(&mut self, description: Option<String>, workflow: String, task: String) -> Result<(), FunctionResponseError> {
        const BUFFER_SIZE: usize = 32 * 1024; // 32KB buffer
        let start_time = OffsetDateTime::now_utc();
        let buf_reader: Box<dyn BufRead> = if let Some(content) = self.payload.content() {
            Box::new(BufReader::with_capacity(
                BUFFER_SIZE,
                content
            ))
        } else if let Some(ref url) = self.payload.url() {
            let file = File::open(url).map_err(|e| {
                FunctionResponseError::new(
                    "Parse".to_string(),
                    400,
                    format!("File open error: {:?}", e)
                )
            })?;
            Box::new(BufReader::with_capacity(BUFFER_SIZE, file))
        } else {
            return Err(FunctionResponseError::new(
                "Parse".to_string(),
                400,
                "No content or URL provided".to_string()
            ));
        };

        match from_reader::<_, ISO20022Message>(buf_reader) {
            Ok(message) => {
                match message.validate() {
                    Ok(()) => {
                        self.data = serde_json::to_value(message).unwrap();
                        let change_log = ChangeLog::new(
                            "data".to_string(),
                            "ISO20022 message parsed".to_string(),
                            None,
                            None
                        );
                        let audit_log = AuditLog::new(
                            workflow.to_string(),
                            task.to_string(),
                            start_time,
                            description.unwrap_or_else(|| "ISO20022 message parsed".to_string()),
                            vec![change_log]
                        );
                        self.audit.push(audit_log);
                        Ok(())
                    }
                    Err(validation_error) => {
                        Err(FunctionResponseError::new(
                            "Parse".to_string(),
                            400,
                            format!("Schema validation error: {:?}", validation_error)
                        ))
                    }
                }
            }
            Err(e) => Err(FunctionResponseError::new(
                "Parse".to_string(),
                400,
                format!("ISO20022 parsing error: {:?}", e)
            )),
        }
    }
}
