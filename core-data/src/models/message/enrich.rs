use time::OffsetDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use datalogic_rs::JsonLogic;

use super::{
    core::Message,
    errors::FunctionResponseError,
    auditlog::{AuditLog, ChangeLog},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EnrichmentConfig {
    pub field: String,

    pub rule: Value,

    pub description: Option<String>,
}

impl Message {
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
        self.version += 1;
        Ok(())
    }
}
