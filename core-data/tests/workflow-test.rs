use serde_json::json;
use core_data::models::workflow::*;
use core_data::models::task::*;
use core_data::models::message::MessageStatus;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_custom_values() {
        let task = Task {
            id: String::from("task_1"),
            name: String::from("Task 1"),
            description: String::from("First task"),
            message_status: MessageStatus::Recieved,
            prev_task: String::from("prev_task"),
            prev_status_code: None,
            condition: json!({"condition": "value"}),
            function: FunctionType::Validate,
            input: json!({"input": "value"}),
        };
        let workflow = Workflow {
            id: String::from("workflow_1"),
            name: String::from("Workflow 1"),
            description: String::from("Test workflow"),
            version: 1,
            tenant: String::from("tenant"),
            origin: String::from("origin"),
            status: WorkflowStatus::Active,
            condition: json!({"condition": "value"}),
            tasks: vec![task.clone()],
            input_topic: String::from("input_topic"),
        };
        assert_eq!(workflow.name, String::from("Workflow 1"));
        assert_eq!(workflow.description, String::from("Test workflow"));
        assert_eq!(workflow.version, 1);
        assert_eq!(workflow.status, WorkflowStatus::Active);
        assert_eq!(workflow.tasks.len(), 1);
        assert_eq!(workflow.tasks[0], task);
    }

    #[test]
    fn test_workflow_empty_tasks() {
        let workflow = Workflow {
            id: String::from("workflow_2"),
            name: String::from("Empty Workflow"),
            description: String::from("Workflow with no tasks"),
            version: 0,
            tenant: String::from("tenant"),
            origin: String::from("origin"),
            status: WorkflowStatus::Draft,
            condition: json!({"condition": "value"}),
            tasks: vec![],
            input_topic: String::from("input_topic"),
        };
        assert_eq!(workflow.name, String::from("Empty Workflow"));
        assert_eq!(workflow.description, String::from("Workflow with no tasks"));
        assert!(workflow.tasks.is_empty());
    }

    #[test]
    fn test_workflow_multiple_tasks() {
        let task1 = Task {
            id: String::from("task_1"),
            name: String::from("Task 1"),
            description: String::from("First task"),
            message_status: MessageStatus::Recieved,
            prev_task: String::from("prev_task"),
            prev_status_code: None,
            condition: json!({"condition": "value"}),
            function: FunctionType::Validate,
            input: json!({"input": "value"}),
        };
        let task2 = Task {
            id: String::from("task_2"),
            name: String::from("Task 2"),
            description: String::from("Second task"),
            message_status: MessageStatus::Recieved,
            prev_task: String::from("prev_task"),
            prev_status_code: None,
            condition: json!({"condition": "value"}),
            function: FunctionType::Enrich,
            input: json!({"input": "value"}),
        };
        let workflow = Workflow {
            id: String::from("workflow_3"),
            name: String::from("Workflow with Multiple Tasks"),
            description: String::from("Workflow containing multiple tasks"),
            version: 0,
            tenant: String::from("tenant"),
            origin: String::from("origin"),
            status: WorkflowStatus::Draft,
            condition: json!({"condition": "value"}),
            tasks: vec![task1.clone(), task2.clone()],
            input_topic: String::from("input_topic"),
        };
        assert_eq!(workflow.name, String::from("Workflow with Multiple Tasks"));
        assert_eq!(workflow.description, String::from("Workflow containing multiple tasks"));
        assert_eq!(workflow.tasks.len(), 2);
        assert_eq!(workflow.tasks[0], task1);
        assert_eq!(workflow.tasks[1], task2);
    }
}