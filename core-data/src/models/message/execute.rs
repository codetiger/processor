use crate::models::task::*;
use crate::models::workflow::*;
use tracing::{debug, error, info, instrument, warn};

use super::errors::WorkflowResponseError;
use super::{
    core::Message,
    errors::FunctionResponseError, EnrichmentRules
};

use super::progress::*;

#[derive(Debug)]
pub struct TaskResult {
    pub status: MessageStatus,
    pub status_code: Option<StatusCode>,
}

impl Message {
    pub fn execute_task(&mut self, workflow_id: String, workflow_version: u16, task: Task) 
    -> Result<TaskResult, FunctionResponseError> {
        debug!(task_id = %task.id, "Executing task");
        
        let result = match task.function {
            FunctionType::Parse => {
                self.parse(Some(task.description), workflow_id, workflow_version, task.id)
                    .map(|_| TaskResult {
                        status: MessageStatus::Processing,
                        status_code: Some(StatusCode::Success)
                    })
            },
            FunctionType::Enrich => {
                let rules: Vec<EnrichmentRules> = serde_json::from_value(task.input).unwrap();
                self.enrich(rules, self.ephemeral_data.clone(), Some(task.description), workflow_id, workflow_version, task.id)
                    .map(|_| TaskResult {
                        status: MessageStatus::Processing,
                        status_code: Some(StatusCode::Success)
                    })
            },
            FunctionType::Fetch => {
                self.fetch(task.input, Some(task.description), workflow_id, workflow_version, task.id)
                    .map(|_| TaskResult {
                        status: MessageStatus::Processing,
                        status_code: Some(StatusCode::Success)
                    })
            },
            _ => Err(FunctionResponseError::new(
                "Execute".to_string(),
                400,
                format!("Function not supported: {:?}", task.function)
            ))
        }?;

        Ok(result)
    }

    #[instrument(skip(self, workflow), fields(workflow_id = %workflow.id))]
    pub fn execute_workflow(&mut self, workflow: &Workflow) -> Result<(), WorkflowResponseError> {
        let start = std::time::Instant::now();
        debug!("Starting workflow execution");
    
        let mut task_executed = true;
        let mut execution_count = 0;
    
        while task_executed {
            task_executed = false;
    
            for task in &workflow.tasks {
                if self.task_match(task.message_status.clone(), &workflow.id, &task.prev_task, 
                    task.prev_status_code.clone(), &task.condition) {
                    
                    debug!(
                        task_id = %task.id,
                        task_name = %task.name,
                        "Executing task"
                    );
    
                    match self.execute_task(workflow.id.clone(), workflow.version, task.clone()) {
                        Ok(task_result) => {
                            // Update progress with task result
                            self.progress = Progress {
                                status: task_result.status.clone(),
                                workflow_id: workflow.id.clone(),
                                prev_task: task.id.clone(),
                                prev_status_code: task_result.status_code,
                                timestamp: time::OffsetDateTime::now_utc(),
                            };
                            
                            debug!(
                                task_id = %task.id,
                                status = ?task_result.status,
                                "Task completed, progress updated"
                            );
                            
                            task_executed = true;
                            execution_count += 1;
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                workflow_id = %workflow.id,
                                task_id = %task.id,
                                "Task execution failed"
                            );
                            // Update progress with failure status
                            self.progress = Progress {
                                status: MessageStatus::Failed,
                                workflow_id: workflow.id.clone(),
                                prev_task: task.id.clone(),
                                prev_status_code: Some(StatusCode::Failure),
                                timestamp: time::OffsetDateTime::now_utc(),
                            };
                            return Err(WorkflowResponseError::new(
                                workflow.id.clone(),
                                workflow.version,
                                500,
                                format!("Task execution failed: {}", e)
                            ));
                        }
                    }
                }
            }
        }
    
        info!(
            workflow_id = %workflow.id,
            duration_ms = start.elapsed().as_millis(),
            tasks_executed = execution_count,
            "Workflow execution completed"
        );
        Ok(())
    }
}
