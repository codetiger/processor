use std::fs::File;
use std::io::{BufReader, BufRead};
use quick_xml::de::from_reader;
use time::OffsetDateTime;

use super::{
    core::Message,
    errors::FunctionResponseError,
    iso20022::ISO20022Message,
    auditlog::{AuditLog, ChangeLog},
};

use tracing::{debug, error, info, instrument};
use std::time::Instant;

impl Message {
    #[instrument(skip(self, description), fields(
        workflow_id = %workflow_id,
        task_id = %task_id
    ))]
    pub fn parse(&mut self, description: Option<String>, workflow_id: String, workflow_version: u16, task_id: String) -> Result<(), FunctionResponseError> {
        const BUFFER_SIZE: usize = 32 * 1024;
        let start = Instant::now();
        let start_time = OffsetDateTime::now_utc();

        debug!("Starting message parsing");

        let buf_reader: Box<dyn BufRead> = if let Some(content) = self.payload.content() {
            debug!(size = content.len(), "Using inline content");
            Box::new(BufReader::with_capacity(BUFFER_SIZE, content))
        } else if let Some(ref url) = self.payload.url() {
            debug!(url = %url, "Opening file for parsing");
            let file = File::open(url).map_err(|e| {
                error!(error = %e, url = %url, "Failed to open file");
                FunctionResponseError::new(
                    "Parse".to_string(),
                    400,
                    format!("File open error: {:?}", e)
                )
            })?;
            Box::new(BufReader::with_capacity(BUFFER_SIZE, file))
        } else {
            error!("No content or URL provided");
            return Err(FunctionResponseError::new(
                "Parse".to_string(),
                400,
                "No content or URL provided".to_string()
            ));
        };

        match from_reader::<_, ISO20022Message>(buf_reader) {
            Ok(message) => {
                debug!("Message parsed, validating schema");
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
                            workflow_id.to_string(),
                            workflow_version,
                            task_id.to_string(),
                            start_time,
                            description.unwrap_or_else(|| "ISO20022 message parsed".to_string()),
                            vec![change_log]
                        );
                        self.audit.push(audit_log);
                        self.version += 1;

                        info!(
                            duration_ms = start.elapsed().as_millis(),
                            "Successfully parsed message"
                        );
                        Ok(())
                    }
                    Err(validation_error) => {
                        error!(error = ?validation_error, "Schema validation failed");
                        Err(FunctionResponseError::new(
                            "Parse".to_string(),
                            400,
                            format!("Schema validation error: {:?}", validation_error)
                        ))
                    }
                }
            }
            Err(e) => {
                error!(error = ?e, "Failed to parse ISO20022 message");
                Err(FunctionResponseError::new(
                    "Parse".to_string(),
                    400,
                    format!("ISO20022 parsing error: {:?}", e)
                ))
            }
        }
    }
}