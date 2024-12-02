use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::OffsetDateTime;
use sonyflake::Sonyflake;

use crate::models::message::payload::*;
use crate::models::message::auditlog::*;
use crate::models::message::errors::FunctionResponseError;
use crate::models::message::progress::*;


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Message {
    #[serde(rename = "id")]
    pub(crate) id: u64,

    pub(crate) parent_id: Option<String>,

    pub(crate) payload: Payload,

    pub(crate) version: u16,

    pub(crate) tenant: String,
    
    pub(crate) origin: String,
    
    pub(crate) data: Value,

    pub(crate) metadata: Value,

    pub(crate) progress: Progress,

    pub(crate) audit: Vec<AuditLog>,

    #[serde(skip)]
    pub(crate) ephemeral_data: Value,

    #[serde(skip)]
    pub(crate) transaction_changes: Option<Vec<(String, Value)>>,
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

    pub fn version(&self) -> u16 {
        self.version
    }

    pub(crate) fn transaction_begin(&mut self, workflow: String, task: String) {
        self.progress.workflow_id = workflow;
        self.progress.prev_task = task;
        self.transaction_changes = Some(Vec::new());
    }

    pub(crate) fn transaction_rollback(&mut self) {
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

    pub(crate) fn transaction_commit(&mut self) {
        self.progress.prev_status_code = Some(StatusCode::Success);
        self.progress.timestamp = OffsetDateTime::now_utc();
        self.transaction_changes = None;
    }

    pub(crate) fn update(&mut self, field_path: &str, new_value: Value) -> Result<(), FunctionResponseError> {
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

    pub fn new(payload: Payload, tenant: String, origin: String, workflow_id: String, workflow_version: u16, task_id: String, message_alias: Option<String>) -> Self {
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
            workflow_id.to_string(), 
            workflow_version,
            task_id.to_string(), 
            start_time,
            description.to_string(),
            vec![change_log]
        );

        Self {
            id,
            parent_id: None,
            payload,
            version: 1,
            tenant,
            origin,
            data: Value::Null,
            metadata: Value::Null,
            progress: Progress {
                status: MessageStatus::Recieved,
                workflow_id: workflow_id.to_string(),
                prev_task: task_id.to_string(),
                prev_status_code: Some(StatusCode::Success),
                timestamp: OffsetDateTime::now_utc(),
            },
            audit: vec![audit],
            transaction_changes: Some(Vec::new()),
            ephemeral_data: Value::Null,
        }
    }
}
