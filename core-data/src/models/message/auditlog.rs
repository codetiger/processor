use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use sonyflake::Sonyflake;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AuditLog {
    #[serde(rename = "id")]
    id: u64,

    #[serde(with = "time::serde::iso8601")]
    pub start_time: OffsetDateTime,

    #[serde(with = "time::serde::iso8601")]
    pub finish_time: OffsetDateTime,

    workflow: Box<str>,

    workflow_version: u16,

    task: Box<str>,

    description: Box<str>,

    hash: Box<str>,

    service: Box<str>,

    instance: Box<str>,

    changes: Box<[ChangeLog]>,
}

impl AuditLog {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn workflow(&self) -> &str {
        &self.workflow
    }

    pub fn task(&self) -> &str {
        &self.task
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn instance(&self) -> &str {
        &self.instance
    }

    pub fn workflow_version(&self) -> u16 {
        self.workflow_version
    }

    pub fn changes(&self) -> &[ChangeLog] {
        &self.changes
    }

    pub fn start_time(&self) -> &OffsetDateTime {
        &self.start_time
    }

    pub fn finish_time(&self) -> &OffsetDateTime {
        &self.finish_time
    }

    pub fn new(workflow: String, workflow_version: u16, task: String, start_time: OffsetDateTime, description: String, changes: Vec<ChangeLog>) -> Self {
        let sf = Sonyflake::new().unwrap();
        let id = sf.next_id().unwrap();
        let timestamp = OffsetDateTime::now_utc();
        AuditLog {
            id,
            start_time,
            finish_time: timestamp,
            workflow: workflow.into_boxed_str(),
            workflow_version,
            task: task.into_boxed_str(),
            description: description.into_boxed_str(),
            hash: String::new().into_boxed_str(),
            service: String::new().into_boxed_str(),
            instance: String::new().into_boxed_str(),
            changes: changes.into_boxed_slice(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ChangeLog {
    field: Box<str>,

    old_value: Option<serde_json::Value>,

    new_value: Option<serde_json::Value>,

    reason: Box<str>,
}

impl ChangeLog {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn old_value(&self) -> Option<&serde_json::Value> {
        self.old_value.as_ref()
    }

    pub fn new_value(&self) -> Option<&serde_json::Value> {
        self.new_value.as_ref()
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
    
    pub fn new(field: String, reason: String, old_value: Option<serde_json::Value>, new_value: Option<serde_json::Value>) -> Self {
        ChangeLog {
            field: field.into_boxed_str(),
            old_value: old_value,
            new_value: new_value,
            reason: reason.into_boxed_str(),
        }
    }
}