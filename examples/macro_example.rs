/**
 * Example file demonstrating Runar macros for service definition
 * 
 * This example demonstrates:
 * 1. Using the service! macro to define services
 * 2. Proper field initialization in service structs
 * 3. Implementing action handlers with the action! macro
 * 4. Subscribing to events with the sub! macro
 * 5. Integration with the node architecture
 */

use anyhow::Result;
use runar_common::types::ValueType;
use runar_macros::{action, service, sub};
use runar_node::{
    node::{Node, NodeConfig},
    services::{abstract_service::ServiceState, RequestContext, ServiceRequest, ServiceResponse},
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use uuid::Uuid;

/// The data models for our task management service
#[derive(Debug, Clone)]
pub struct Task {
    id: String,
    title: String,
    description: Option<String>,
    completed: bool,
    created_at: u64,
}

/// ✅ CORRECT: Define a service using the service! macro
/// Note the field initializations in the struct definition
#[service(
    name = "task_manager",
    description = "A service for managing tasks",
    version = "1.0.0"
)]
pub struct TaskManagerService {
    // Proper initialization of fields
    tasks: Arc<Mutex<HashMap<String, Task>>> = Arc::new(Mutex::new(HashMap::new())),
    task_count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0)),
}

// Regular impl block for custom service methods
impl TaskManagerService {
    // Helper method to get current timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    // Helper method to get all tasks as a vector
    fn get_all_tasks(&self) -> Vec<Task> {
        let tasks = self.tasks.lock().unwrap();
        tasks.values().cloned().collect()
    }

    // Helper method to find a task by ID
    fn find_task(&self, task_id: &str) -> Option<Task> {
        let tasks = self.tasks.lock().unwrap();
        tasks.get(task_id).cloned()
    }

    // Helper method to add a new task
    fn add_task(&self, title: String, description: Option<String>) -> Task {
        let task_id = Uuid::new_v4().to_string();
        let new_task = Task {
            id: task_id.clone(),
            title,
            description,
            completed: false,
            created_at: Self::current_timestamp(),
        };

        // Store the task
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task_id, new_task.clone());

        // Update task count
        let mut count = self.task_count.lock().unwrap();
        *count += 1;

        new_task
    }

    // Helper method to update a task
    fn update_task(&self, task_id: &str, title: Option<String>, description: Option<String>, completed: Option<bool>) -> Option<Task> {
        let mut tasks = self.tasks.lock().unwrap();
        
        if let Some(task) = tasks.get_mut(task_id) {
            if let Some(new_title) = title {
                task.title = new_title;
            }
            
            if let Some(new_description) = description {
                task.description = Some(new_description);
            }
            
            if let Some(new_completed) = completed {
                task.completed = new_completed;
            }
            
            return Some(task.clone());
        }
        
        None
    }

    // Helper method to delete a task
    fn delete_task(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        
        if tasks.remove(task_id).is_some() {
            let mut count = self.task_count.lock().unwrap();
            *count = count.saturating_sub(1);
            return true;
        }
        
        false
    }
}

/// ✅ CORRECT: Define service actions using the action! macro
/// Each action maps to an operation that can be invoked via the request-based API
impl TaskManagerService {
    // Action to list all tasks
    #[action]
    async fn list_tasks(&self, _request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        let tasks = self.get_all_tasks();
        let task_count = tasks.len();
        
        // Convert tasks to ValueType for the response
        let tasks_json: Vec<serde_json::Value> = tasks
            .into_iter()
            .map(|task| {
                json!({
                    "id": task.id,
                    "title": task.title,
                    "description": task.description,
                    "completed": task.completed,
                    "created_at": task.created_at
                })
            })
            .collect();
        
        // Create the response
        let response_data = json!({
            "tasks": tasks_json,
            "count": task_count
        });
        
        ServiceResponse::success("Tasks retrieved successfully", Some(ValueType::Json(response_data)))
    }
    
    // Action to create a new task
    #[action]
    async fn create_task(&self, request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        // Extract parameters from request
        if let Some(params) = request.params {
            match params {
                ValueType::Json(json) => {
                    // Extract task properties
                    let title = json.get("title")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Title is required"))?
                        .to_string();
                    
                    let description = json.get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    // Create the task
                    let new_task = self.add_task(title, description);
                    
                    // Publish task created event
                    request.context.publish(
                        "tasks/events/created",
                        ValueType::Json(json!({
                            "task_id": new_task.id,
                            "timestamp": Self::current_timestamp()
                        }))
                    ).await?;
                    
                    // Return the created task
                    let response_data = json!({
                        "task": {
                            "id": new_task.id,
                            "title": new_task.title,
                            "description": new_task.description,
                            "completed": new_task.completed,
                            "created_at": new_task.created_at
                        }
                    });
                    
                    ServiceResponse::success("Task created successfully", Some(ValueType::Json(response_data)))
                },
                _ => ServiceResponse::error("Invalid request format")
            }
        } else {
            ServiceResponse::error("No parameters provided")
        }
    }
    
    // Action to get a specific task by ID
    #[action]
    async fn get_task(&self, request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        // Extract task ID from request
        if let Some(params) = request.params {
            match params {
                ValueType::Json(json) => {
                    // Get the task ID
                    let task_id = json.get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Task ID is required"))?;
                    
                    // Find the task
                    if let Some(task) = self.find_task(task_id) {
                        // Return the task
                        let response_data = json!({
                            "task": {
                                "id": task.id,
                                "title": task.title,
                                "description": task.description,
                                "completed": task.completed,
                                "created_at": task.created_at
                            }
                        });
                        
                        ServiceResponse::success("Task retrieved successfully", Some(ValueType::Json(response_data)))
                    } else {
                        ServiceResponse::error("Task not found")
                    }
                },
                _ => ServiceResponse::error("Invalid request format")
            }
        } else {
            ServiceResponse::error("No parameters provided")
        }
    }
    
    // Action to update a task
    #[action]
    async fn update_task(&self, request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        // Extract parameters from request
        if let Some(params) = request.params {
            match params {
                ValueType::Json(json) => {
                    // Get the task ID
                    let task_id = json.get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Task ID is required"))?;
                    
                    // Get update fields
                    let title = json.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let description = json.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let completed = json.get("completed").and_then(|v| v.as_bool());
                    
                    // Update the task
                    if let Some(updated_task) = self.update_task(task_id, title, description, completed) {
                        // Publish task updated event if completed status changed
                        if let Some(true) = completed {
                            request.context.publish(
                                "tasks/events/completed",
                                ValueType::Json(json!({
                                    "task_id": updated_task.id,
                                    "timestamp": Self::current_timestamp()
                                }))
                            ).await?;
                        }
                        
                        // Return the updated task
                        let response_data = json!({
                            "task": {
                                "id": updated_task.id,
                                "title": updated_task.title,
                                "description": updated_task.description,
                                "completed": updated_task.completed,
                                "created_at": updated_task.created_at
                            }
                        });
                        
                        ServiceResponse::success("Task updated successfully", Some(ValueType::Json(response_data)))
                    } else {
                        ServiceResponse::error("Task not found")
                    }
                },
                _ => ServiceResponse::error("Invalid request format")
            }
        } else {
            ServiceResponse::error("No parameters provided")
        }
    }
    
    // Action to delete a task
    #[action]
    async fn delete_task(&self, request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        // Extract task ID from request
        if let Some(params) = request.params {
            match params {
                ValueType::Json(json) => {
                    // Get the task ID
                    let task_id = json.get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Task ID is required"))?;
                    
                    // Delete the task
                    if self.delete_task(task_id) {
                        // Publish task deleted event
                        request.context.publish(
                            "tasks/events/deleted",
                            ValueType::Json(json!({
                                "task_id": task_id,
                                "timestamp": Self::current_timestamp()
                            }))
                        ).await?;
                        
                        ServiceResponse::success("Task deleted successfully", None)
                    } else {
                        ServiceResponse::error("Task not found")
                    }
                },
                _ => ServiceResponse::error("Invalid request format")
            }
        } else {
            ServiceResponse::error("No parameters provided")
        }
    }
    
    // Subscribe to events during service initialization
    #[sub(topic = "tasks/commands/purge")]
    async fn handle_purge_command(&self, _payload: ValueType, context: &RequestContext) -> Result<(), anyhow::Error> {
        println!("Received purge command, clearing all tasks");
        
        // Clear all tasks
        let mut tasks = self.tasks.lock().unwrap();
        let task_count = tasks.len();
        tasks.clear();
        
        // Reset task count
        let mut count = self.task_count.lock().unwrap();
        *count = 0;
        
        // Publish purge completed event
        context.publish(
            "tasks/events/purged",
            ValueType::Json(json!({
                "purged_count": task_count,
                "timestamp": Self::current_timestamp()
            }))
        ).await?;
        
        Ok(())
    }
}

// Define an analytics service to demonstrate event handling
#[service(
    name = "task_analytics",
    description = "A service that analyzes task activities",
    version = "1.0.0"
)]
pub struct TaskAnalyticsService {
    total_created: Arc<Mutex<u32>> = Arc::new(Mutex::new(0)),
    total_completed: Arc<Mutex<u32>> = Arc::new(Mutex::new(0)),
    total_deleted: Arc<Mutex<u32>> = Arc::new(Mutex::new(0)),
}

impl TaskAnalyticsService {
    // Action to get analytics data
    #[action]
    async fn get_analytics(&self, _request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        let created = *self.total_created.lock().unwrap();
        let completed = *self.total_completed.lock().unwrap();
        let deleted = *self.total_deleted.lock().unwrap();
        
        let response_data = json!({
            "metrics": {
                "total_created": created,
                "total_completed": completed,
                "total_deleted": deleted,
                "completion_rate": if created > 0 { (completed as f64 / created as f64) * 100.0 } else { 0.0 }
            }
        });
        
        ServiceResponse::success("Analytics retrieved successfully", Some(ValueType::Json(response_data)))
    }
    
    // Subscribe to task created events
    #[sub(topic = "tasks/events/created")]
    async fn handle_task_created(&self, _payload: ValueType, _context: &RequestContext) -> Result<(), anyhow::Error> {
        let mut count = self.total_created.lock().unwrap();
        *count += 1;
        println!("Analytics: Task created event received. Total created: {}", *count);
        Ok(())
    }
    
    // Subscribe to task completed events
    #[sub(topic = "tasks/events/completed")]
    async fn handle_task_completed(&self, _payload: ValueType, _context: &RequestContext) -> Result<(), anyhow::Error> {
        let mut count = self.total_completed.lock().unwrap();
        *count += 1;
        println!("Analytics: Task completed event received. Total completed: {}", *count);
        Ok(())
    }
    
    // Subscribe to task deleted events
    #[sub(topic = "tasks/events/deleted")]
    async fn handle_task_deleted(&self, _payload: ValueType, _context: &RequestContext) -> Result<(), anyhow::Error> {
        let mut count = self.total_deleted.lock().unwrap();
        *count += 1;
        println!("Analytics: Task deleted event received. Total deleted: {}", *count);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Runar Macros Example");
    
    // Create a temporary directory for node data
    let temp_dir = tempdir()?;
    let node_path = temp_dir.path().to_string_lossy().to_string();
    
    // Create and configure the node
    let config = NodeConfig {
        network_id: "test-network".to_string(),
        node_path: node_path.clone(),
        db_path: format!("{}/db", node_path),
        state_path: Some(format!("{}/state", node_path)),
        node_id: Some("test-node".to_string()),
        bootstrap_nodes: None,
        listen_addr: None,
        p2p_config: None,
        test_network_ids: None,
    };
    
    // Create and initialize the node
    let mut node = Node::new(config).await?;
    node.init().await?;
    
    // Create and register services
    println!("Registering services...");
    let task_service = TaskManagerService::new();
    let analytics_service = TaskAnalyticsService::new();
    
    // ✅ CORRECT: Register services using add_service
    node.add_service(task_service).await?;
    node.add_service(analytics_service).await?;
    
    // Start the node
    println!("Starting node...");
    node.start().await?;
    
    // Verify services are registered
    let response = node.request(
        "internal/registry/list_services",
        ValueType::Null,
    ).await?;
    
    if let Some(ValueType::Map(data)) = response.data {
        if let Some(ValueType::Array(services)) = data.get("services") {
            println!("Registered services:");
            for service in services {
                if let ValueType::String(name) = service {
                    println!("- {}", name);
                }
            }
        }
    }
    
    // Create some tasks
    println!("\nCreating tasks...");
    for i in 1..=5 {
        let task_data = json!({
            "title": format!("Example Task {}", i),
            "description": format!("This is description for task {}", i)
        });
        
        node.request(
            "task_manager/create_task",
            ValueType::Json(task_data),
        ).await?;
    }
    
    // Complete a couple of tasks
    println!("\nCompleting tasks...");
    for i in 1..=2 {
        let task_data = json!({
            "id": format!("{}", i),
            "completed": true
        });
        
        node.request(
            "task_manager/update_task",
            ValueType::Json(task_data),
        ).await?;
    }
    
    // Delete a task
    println!("\nDeleting a task...");
    let delete_data = json!({
        "id": "3"
    });
    
    node.request(
        "task_manager/delete_task",
        ValueType::Json(delete_data),
    ).await?;
    
    // Get all tasks
    println!("\nRetrieving all tasks...");
    let list_response = node.request(
        "task_manager/list_tasks",
        ValueType::Null,
    ).await?;
    
    println!("Task list response: {:?}", list_response);
    
    // Get analytics
    println!("\nRetrieving analytics...");
    let analytics_response = node.request(
        "task_analytics/get_analytics",
        ValueType::Null,
    ).await?;
    
    println!("Analytics response: {:?}", analytics_response);
    
    // Send a purge command via the event system
    println!("\nPurging all tasks via event command...");
    node.publish(
        "tasks/commands/purge",
        ValueType::Null,
    ).await?;
    
    // Wait for events to be processed
    println!("Waiting for events to be processed...");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Get final analytics
    println!("\nRetrieving final analytics...");
    let final_analytics = node.request(
        "task_analytics/get_analytics",
        ValueType::Null,
    ).await?;
    
    println!("Final analytics: {:?}", final_analytics);
    
    println!("\nExample completed successfully!");
    Ok(())
} 