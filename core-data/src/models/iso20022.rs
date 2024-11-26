use serde::{Deserialize, Serialize};
use open_payments_iso20022::document::Document;
use open_payments_common::ValidationError;

#[derive(Serialize, Deserialize)]
#[serde(rename = "Document")]
pub struct ISO20022Message {
    #[serde(rename( deserialize = "$value" ))]
    pub document: Document,
}

impl ISO20022Message {
	pub fn validate(&self) -> Result<(), ValidationError> {
        self.document.validate()?;
		Ok(())
	}
}
