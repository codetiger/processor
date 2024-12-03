use std::fmt;

#[derive(Debug)]
pub struct FunctionResponseError {
    pub function: String,
    pub code: u32,
    pub message: String,
}

impl FunctionResponseError {
    pub fn new(function: String, code: u32, message: String) -> Self {
        FunctionResponseError { function, code, message }
    }
}

impl From<FunctionResponseError> for WorkflowResponseError {
    fn from(err: FunctionResponseError) -> Self {
        WorkflowResponseError {
            workflow_id: "unknown".to_string(), // This will be updated in execute_workflow
            version: 0,                         // This will be updated in execute_workflow
            code: 500,
            desciption: format!("Function error: {} ({})", err.message, err.function),
        }
    }
}

impl fmt::Display for FunctionResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Function '{}' failed (code {}): {}", 
            self.function, self.code, self.message)
    }
}

pub struct WorkflowResponseError {
    pub workflow_id: String,
    pub version: u16,
    pub code: u16,
    pub desciption: String,
}

impl WorkflowResponseError {
    pub fn new(workflow_id: String, version: u16, code: u16, desciption: String) -> Self {
        WorkflowResponseError { workflow_id, version, code, desciption }
    }
}

impl fmt::Display for WorkflowResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Workflow error - ID: {}, Version: {}, Code: {}, Message: {}",
            self.workflow_id, self.version, self.code, self.desciption
        )
    }
}