use serde_json::Value;
use datalogic_rs::JsonLogic;

use super::core::Message;
use super::progress::MessageStatus;
use super::progress::StatusCode;

impl Message {
    pub fn workflow_match(&self, tenant: &str, origin: &str, condition: &Value) -> bool {
        // Check tenant and origin match
        if self.tenant != tenant || self.origin != origin {
            return false;
        }

        // If condition is null or null-like, return true
        if condition.is_null() {
            return true;
        }

        // Evaluate condition against metadata
        let logic = JsonLogic::new();
        let result = logic
            .apply(condition, &self.metadata)
            .unwrap_or(Value::Bool(false));
            
        result.as_bool().unwrap_or(false)
    }

    pub fn task_match(&self, status: MessageStatus, workflow_id: &str, prev_task: &str, prev_status_code: Option<StatusCode>, condition: &Value) -> bool {
        if workflow_id != self.progress.workflow_id || prev_task != self.progress.prev_task || prev_status_code != self.progress.prev_status_code || status != self.progress.status {
            return false;
        }

        // If condition is null or null-like, return true
        if condition.is_null() {
            return true;
        }

        // Evaluate condition against data
        let logic = JsonLogic::new();
        let result = logic
            .apply(condition, &self.metadata)
            .unwrap_or(Value::Bool(false));
            
        result.as_bool().unwrap_or(false)
    }
}
