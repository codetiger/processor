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


impl Message {
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
                        self.version += 1;
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
