use serde_json::Value;
use datalogic_rs::JsonLogic;
use tracing::{debug, trace, instrument};
use std::time::Instant;

use super::core::Message;
use super::progress::MessageStatus;
use super::progress::StatusCode;

impl Message {
    #[instrument(skip(self, tenant, origin, condition), fields(
        message_id = %self.id,
        tenant = %tenant,
        origin = %origin
    ))]
    pub fn workflow_match(&self, tenant: &str, origin: &str, condition: &Value) -> bool {
        let start = Instant::now();

        // Check tenant and origin match
        if self.tenant != tenant || self.origin != origin {
            debug!(
                message_tenant = %self.tenant,
                message_origin = %self.origin,
                "Workflow match failed: tenant/origin mismatch"
            );
            return false;
        }

        // If condition is null or null-like, return true
        if condition.is_null() {
            debug!("No condition specified, automatic match");
            return true;
        }

        // Evaluate condition against metadata
        let logic = JsonLogic::new();
        trace!(
            condition = ?condition,
            metadata = ?self.metadata,
            "Evaluating workflow condition"
        );

        let result = logic
            .apply(condition, &self.metadata)
            .unwrap_or(Value::Bool(false));
            
        let matches = result.as_bool().unwrap_or(false);
        debug!(
            matches = matches,
            duration_ms = start.elapsed().as_millis(),
            "Workflow match completed"
        );
        matches
    }

    #[instrument(skip(self, workflow_id, prev_task, condition), fields(
        message_id = %self.id,
        workflow_id = %workflow_id,
        status = ?status
    ))]
    pub fn task_match(&self, status: MessageStatus, workflow_id: &str, prev_task: &str, prev_status_code: Option<StatusCode>, condition: &Value) -> bool {
        let start = Instant::now();

        // Check basic conditions
        if workflow_id != self.progress.workflow_id || 
           prev_task != self.progress.prev_task || 
           prev_status_code != self.progress.prev_status_code || 
           status != self.progress.status {
            trace!(
                current_status = ?self.progress.status,
                current_task = %self.progress.prev_task,
                "Task match failed: status/task mismatch"
            );
            return false;
        }

        // If condition is null or null-like, return true
        if condition.is_null() {
            debug!("No condition specified, automatic match");
            return true;
        }

        // Evaluate condition against data
        let logic = JsonLogic::new();
        trace!(
            condition = ?condition,
            metadata = ?self.metadata,
            "Evaluating task condition"
        );

        let result = logic
            .apply(condition, &self.metadata)
            .unwrap_or(Value::Bool(false));
            
        let matches = result.as_bool().unwrap_or(false);
        debug!(
            matches = matches,
            duration_ms = start.elapsed().as_millis(),
            "Task match completed"
        );
        matches
    }
}