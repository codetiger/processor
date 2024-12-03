use time::OffsetDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use datalogic_rs::JsonLogic;
use tracing::{debug, error, info, instrument, warn};
use std::time::Instant;

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
    #[instrument(skip(self, rules, data, description), fields(
        workflow_id = %workflow_id,
        task_id = %task_id,
        rule_count = rules.len()
    ))]
    pub fn enrich(
        &mut self,
        rules: Vec<EnrichmentRules>,
        data: serde_json::Value,
        description: Option<String>,
        workflow_id: String,
        workflow_version: u16,
        task_id: String
    ) -> Result<(), FunctionResponseError> {
        let start = Instant::now();
        let start_time = OffsetDateTime::now_utc();
        let logic = JsonLogic::new();
        let mut changes = Vec::new();
        
        debug!(
            rule_count = rules.len(),
            "Starting message enrichment"
        );

        // Begin transaction
        self.transaction_begin(workflow_id.clone(), task_id.clone());
        debug!("Transaction started");

        for (idx, rule) in rules.into_iter().enumerate() {
            debug!(
                rule_index = idx,
                field = %rule.field,
                "Applying enrichment rule"
            );

            let value = match logic.apply(&rule.logic, &data) {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        error = ?e,
                        rule_index = idx,
                        field = %rule.field,
                        "Rule application failed"
                    );
                    self.transaction_rollback();
                    debug!("Transaction rolled back due to rule application failure");
                    return Err(FunctionResponseError::new(
                        "Enrichment".to_string(),
                        400,
                        format!("Rule application failed: {:?}", e)
                    ));
                }
            };

            // Update with transaction support
            if let Err(e) = self.update(&rule.field, value.clone()) {
                error!(
                    error = ?e,
                    field = %rule.field,
                    "Field update failed"
                );
                self.transaction_rollback();
                debug!("Transaction rolled back due to update failure");
                return Err(e);
            }

            debug!(
                field = %rule.field,
                "Field updated successfully"
            );

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
        debug!("Transaction committed successfully");

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

        info!(
            duration_ms = start.elapsed().as_millis(),
            "Message enrichment completed successfully"
        );
        Ok(())
    }
}