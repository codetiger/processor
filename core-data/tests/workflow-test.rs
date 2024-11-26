use serde_json::json;
use core_data::models::workflow::*;
use core_data::models::task::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_custom_values() {
        let task = Task {
            task_id: String::from("task_1"),
            name: String::from("Task 1"),
            description: String::from("First task"),
            condition: json!({"condition": "value"}),
            function: FunctionType::Validate,
            input: json!({"input": "value"}),
        };
        let workflow = Workflow {
            name: String::from("Workflow 1"),
            description: String::from("Test workflow"),
            version: 1,
            tags: vec![String::from("tag1"), String::from("tag2")],
            status: WorkflowStatus::Active,
            tasks: vec![task.clone()],
            condition: json!({"condition": "value"}),
        };
        assert_eq!(workflow.name, String::from("Workflow 1"));
        assert_eq!(workflow.description, String::from("Test workflow"));
        assert_eq!(workflow.version, 1);
        assert_eq!(workflow.tags, vec![String::from("tag1"), String::from("tag2")]);
        assert_eq!(workflow.status, WorkflowStatus::Active);
        assert_eq!(workflow.tasks.len(), 1);
        assert_eq!(workflow.tasks[0], task);
    }

    #[test]
    fn test_workflow_empty_tasks() {
        let workflow = Workflow {
            name: String::from("Empty Workflow"),
            description: String::from("Workflow with no tasks"),
            version: 0,
            tags: vec![],
            status: WorkflowStatus::Draft,
            tasks: vec![],
            condition: json!({"condition": "value"}),
        };
        assert_eq!(workflow.name, String::from("Empty Workflow"));
        assert_eq!(workflow.description, String::from("Workflow with no tasks"));
        assert!(workflow.tasks.is_empty());
    }

    #[test]
    fn test_workflow_multiple_tasks() {
        let task1 = Task {
            task_id: String::from("task_1"),
            name: String::from("Task 1"),
            description: String::from("First task"),
            condition: json!({"condition": "value"}),
            function: FunctionType::Validate,
            input: json!({"input": "value"}),
        };
        let task2 = Task {
            task_id: String::from("task_2"),
            name: String::from("Task 2"),
            description: String::from("Second task"),
            condition: json!({"condition": "value"}),
            function: FunctionType::Enrich,
            input: json!({"input": "value"}),
        };
        let workflow = Workflow {
            name: String::from("Workflow with Multiple Tasks"),
            description: String::from("Workflow containing multiple tasks"),
            version: 0,
            tags: vec![],
            status: WorkflowStatus::Draft,
            tasks: vec![task1.clone(), task2.clone()],
            condition: json!({"condition": "value"}),
        };
        assert_eq!(workflow.name, String::from("Workflow with Multiple Tasks"));
        assert_eq!(workflow.description, String::from("Workflow containing multiple tasks"));
        assert_eq!(workflow.tasks.len(), 2);
        assert_eq!(workflow.tasks[0], task1);
        assert_eq!(workflow.tasks[1], task2);
    }
}