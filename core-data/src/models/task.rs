use serde::Deserialize;

use crate::models::message::MessageStatus;
use crate::models::message::StatusCode;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Task {
    pub id: String,

    pub name: String,

    pub description: String,

    pub message_status: MessageStatus, 
    
    pub prev_task: String,
    
    pub prev_status_code: Option<StatusCode>,

    pub condition: serde_json::Value,

    pub function: FunctionType,

    pub input: serde_json::Value,
}


#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum FunctionType {
    Parse,
    Validate,
    Fetch,
    Enrich,
    Publish,
}
