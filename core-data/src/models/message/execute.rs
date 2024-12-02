use crate::models::task::*;
use crate::models::workflow::*;

use super::errors::WorkflowResponseError;
use super::{
    core::Message,
    errors::FunctionResponseError, EnrichmentRules,
};

impl Message {
    pub fn execute_task(&mut self, workflow_id: String, workflow_version: u16, task: Task) -> Result<(), FunctionResponseError> {
        match task.function {
            FunctionType::Parse => {
                self.parse(Some(task.description), workflow_id, workflow_version, task.id)
            },
            FunctionType::Enrich => {
                let rules: Vec<EnrichmentRules> = serde_json::from_value(task.input).unwrap();
                self.enrich(rules, self.ephemeral_data.clone(), Some(task.description), workflow_id, workflow_version, task.id)
            },
            _ => {
                Err(FunctionResponseError::new(
                    "Execute".to_string(),
                    400,
                    format!("Function not supported: {:?}", task.function)
                ))
            }
        }
    }

    pub fn execute_workflow(&mut self, workflow: &Workflow) -> Result<(), WorkflowResponseError> {
        let mut task_executed = true;
    
        while task_executed {
            task_executed = false;
    
            for task in &workflow.tasks {
                if self.task_match(task.message_status.clone(), &workflow.id, &task.prev_task, task.prev_status_code.clone(), &task.condition) {
                    self.execute_task(workflow.id.clone(), workflow.version, task.clone()).unwrap();
                    task_executed = true;
                }
            }

            if self.audit.len() > 100 {
                return Err(WorkflowResponseError::new(
                    workflow.id.clone(),
                    workflow.version,
                    500,
                    "Audit log exceeded maximum size".to_string()
                ));
            }
        }
    
        Ok(())
    }
}
