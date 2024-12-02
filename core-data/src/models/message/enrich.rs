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
pub struct EnrichmentRules {
    pub field: String,

    pub logic: Value,

    pub description: Option<String>,
}

impl Message {
    pub fn enrich(&mut self, rules: Vec<EnrichmentRules>, data: serde_json::Value, description: Option<String>, workflow_id: String, workflow_version: u16, task_id: String) -> Result<(), FunctionResponseError> {
        let start_time = OffsetDateTime::now_utc();
        let logic = JsonLogic::new();
        let mut changes = Vec::new();
        
        // Begin transaction
        self.transaction_begin(workflow_id.clone(), task_id.clone());

        for rule in rules {
            let value = match logic.apply(&rule.logic, &data) {
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
            if let Err(e) = self.update(&rule.field, value.clone()) {
                self.transaction_rollback();
                return Err(e);
            }

            // Record change for audit
            changes.push(ChangeLog::new(
                rule.field.to_string(),
                rule.description.unwrap_or_else(|| format!("Enriched field {}", rule.field)),
                None,
                Some(value)
            ));
        }

        // Commit transaction
        self.transaction_commit();

        // Create audit log
        let audit_log = AuditLog::new(
            workflow_id.to_string(),
            workflow_version,
            task_id.to_string(),
            start_time,
            description.unwrap_or_else(|| "Enrichment applied".to_string()),
            changes
        );
        self.audit.push(audit_log);
        self.version += 1;
        Ok(())
    }
}
