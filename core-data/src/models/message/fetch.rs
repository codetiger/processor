use time::OffsetDateTime;
use tracing::{debug, info, instrument, warn};
use std::time::Instant;

use super::{
    core::Message,
    errors::FunctionResponseError,
    auditlog::AuditLog,
};

impl Message {
    #[instrument(skip(self, data, description), fields(
        workflow_id = %workflow_id,
        task_id = %task_id
    ))]
    pub fn fetch(
        &mut self,
        data: serde_json::Value,
        description: Option<String>,
        workflow_id: String,
        workflow_version: u16,
        task_id: String
    ) -> Result<(), FunctionResponseError> {
        let start = Instant::now();
        let start_time = OffsetDateTime::now_utc();
        
        debug!(
            "Starting to run fetch function"
        );

        self.ephemeral_data = data;

        // Create audit log
        let audit_log = AuditLog::new(
            workflow_id.to_string(),
            workflow_version,
            task_id.to_string(),
            start_time,
            description.unwrap_or_else(|| "Fetch applied".to_string()),
            vec![]
        );
        self.audit.push(audit_log);
        self.version += 1;

        info!(
            duration_ms = start.elapsed().as_millis(),
            "Fetch function completed successfully"
        );
        Ok(())
    }
}