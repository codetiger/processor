use serde::Deserialize;
use crate::models::task::*;


#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Workflow {
    pub id: String,

    pub name: String,

    pub description: String,

    pub version: u16,

    pub tenant: String,

    pub origin: String,

    pub status: WorkflowStatus,

    pub condition: serde_json::Value,

    pub tasks: Vec<Task>,

    pub input_topic: String,

    pub persist_on_complete: bool,
}


#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum WorkflowStatus {
    Draft,
    Active,
    Deprecated,
}


