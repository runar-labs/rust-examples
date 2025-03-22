/**
 * Example file demonstrating the Runar Node API
 * 
 * This example demonstrates:
 * 1. Creating and configuring a node
 * 2. Creating a custom service that handles events
 * 3. Proper service registration using add_service()
 * 4. Making service requests using the request-based API
 * 5. Using vmap! for clean parameter extraction
 * 6. Proper service implementation following the AbstractService trait
 * 7. Event handling and storage
 */

use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use runar_common::types::ValueType;
use runar_node::{
    services::{
        abstract_service::{AbstractService, ServiceMetadata, ServiceState},
        ResponseStatus, ServiceRequest, ServiceResponse,
        RequestContext,
    },
    node::Node,
    node::NodeConfig,
};
use serde_json::json;
use tempfile::tempdir;

// Event struct to represent events handled by our service
pub struct Event {
    id: String,
    event_type: String,
    data: Option<ValueType>,
    timestamp: SystemTime,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Runar Node API Example");
    
    //--------------------------
    // 1. Node Configuration
    //--------------------------
    
    // Create a temporary directory for the node
    let temp_dir = tempdir()?;
    let node_path = temp_dir.path().to_string_lossy().to_string();
    
    // Configure and create the node with the correct fields
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
    
    //--------------------------
    // 2. Node Initialization
    //--------------------------
    
    println!("Creating and initializing node");
    let mut node = Node::new(config).await?;
    node.init().await?;
    
    //--------------------------
    // 3. Service Registration
    //--------------------------
    
    println!("Registering EventHandlerService");
    let service = EventHandlerService::new();
    
    // ✅ CORRECT: Register the service using add_service method
    node.add_service(service).await?;
    
    //--------------------------
    // 4. Node Startup
    //--------------------------
    
    println!("Starting node");
    node.start().await?;
    
    //--------------------------
    // 5. Service Registry Access
    //--------------------------
    
    // ✅ CORRECT: Using the request-based API to access registry information
    println!("Checking registered services");
    let response = node.request(
        "internal/registry/list_services",
        ValueType::Null,
    ).await?;
    
    // ✅ CORRECT: Using vmap! to extract data with defaults
    let services = vmap!(response.data, "services" => Vec::<String>::new());
    assert!(services.contains(&"event_handler".to_string()), "Service not registered correctly");
    
    //--------------------------
    // 6. Service Interaction
    //--------------------------
    
    // Create an event payload
    let event_data = json!({
        "type": "test_event",
        "data": {
            "message": "Hello, Runar!",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    });
    
    // ✅ CORRECT: Using request-based API for service interaction
    println!("Storing test event...");
    let event_response = node.request(
        "event_handler/store_event",
        ValueType::Json(event_data),
    ).await?;
    
    println!("Event response: {:?}", event_response);
    
    // Wait for the event to be processed
    println!("Waiting for event to be processed...");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Retrieve stored events
    println!("Retrieving stored events...");
    let events_response = node.request(
        "event_handler/get_events",
        ValueType::Null,
    ).await?;
    
    // ✅ CORRECT: Using vmap! for data extraction with defaults
    let events_data = vmap!(events_response.data, "events" => Vec::<ValueType>::new());
    let count = vmap!(events_response.data, "count" => 0);
    
    println!("Current events: {:?}", events_data);
    println!("Total events: {}", count);
    
    println!("Example completed successfully!");
    
    Ok(())
}

/// EventHandlerService demonstrates a simple Service implementation
/// that manages event storage and retrieval.
///
/// This service follows best practices for implementing the AbstractService trait
/// and event handling by:
/// - Subscribing to events during initialization
/// - Processing events through handlers
/// - Using context.publish for outgoing events
/// - Clean request handling for retrieving stored data
pub struct EventHandlerService {
    name: String,
    path: String,
    state: Mutex<ServiceState>,
    events: Arc<Mutex<Vec<Event>>>,
}

impl EventHandlerService {
    /// Creates a new instance of the EventHandlerService
    pub fn new() -> Self {
        Self {
            name: "event_handler".to_string(),
            path: "event_handler".to_string(),
            state: Mutex::new(ServiceState::Created),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    // Helper method to add an event to the internal store
    async fn add_event(&self, event: Event) -> Result<(), anyhow::Error> {
        let mut events = self.events.lock().unwrap();
        events.push(event);
        println!("Event added. Total events: {}", events.len());
        Ok(())
    }

    // Helper method to process an incoming event payload
    async fn process_event(&self, payload: ValueType) -> Result<(), anyhow::Error> {
        println!("Processing event: {:?}", payload);
        
        // Extract event data from payload
        let event = match payload {
            ValueType::Json(json) => {
                // Parse event from JSON
                let event_type = json.get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                    
                let data = json.get("data")
                    .unwrap_or(&serde_json::Value::Null)
                    .clone();
                    
                Event {
                    id: Uuid::new_v4().to_string(),
                    event_type: event_type.to_string(),
                    data: Some(ValueType::Json(data)),
                    timestamp: SystemTime::now(),
                }
            },
            ValueType::Map(ref map) => {
                // Parse event from Map
                let event_type = match map.get("type") {
                    Some(ValueType::String(s)) => s.clone(),
                    _ => "unknown".to_string(),
                };
                
                Event {
                    id: Uuid::new_v4().to_string(),
                    event_type,
                    data: Some(payload),
                    timestamp: SystemTime::now(),
                }
            },
            _ => {
                // For other types, create a simple event
                Event {
                    id: Uuid::new_v4().to_string(),
                    event_type: "raw".to_string(),
                    data: Some(payload),
                    timestamp: SystemTime::now(),
                }
            }
        };
        
        // Store the event
        self.add_event(event).await
    }
}

// Implement Clone trait to support event subscriptions
impl Clone for EventHandlerService {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            path: self.path.clone(),
            state: Mutex::new(*self.state.lock().unwrap()),
            events: self.events.clone(),
        }
    }
}

#[async_trait]
impl AbstractService for EventHandlerService {
    /// Returns the service name
    fn name(&self) -> &str {
        &self.name
    }
    
    /// Returns the service path
    fn path(&self) -> &str {
        &self.path
    }
    
    /// Returns the current service state
    fn state(&self) -> ServiceState {
        *self.state.lock().unwrap()
    }
    
    /// Returns a description of the service
    fn description(&self) -> &str {
        "A service for handling and storing events"
    }
    
    /// Initializes the service when the node starts
    /// This is where subscriptions should be set up
    async fn init(&mut self, context: &RequestContext) -> Result<(), anyhow::Error> {
        println!("Initializing Event Handler Service");
        
        // ✅ CORRECT: Subscribe to events during initialization
        let self_clone = self.clone();
        
        // Subscribe to a specific topic - we'll use "app/events" for this example
        context.subscribe("app/events", move |payload| {
            let service = self_clone.clone();
            Box::pin(async move {
                service.process_event(payload).await
            })
        }).await?;
        
        println!("Event subscription registered");
        
        // Update state
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Initialized;
        
        Ok(())
    }
    
    /// Starts the service and marks it as running
    async fn start(&mut self) -> Result<(), anyhow::Error> {
        println!("Starting Event Handler Service");
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Running;
        Ok(())
    }
    
    /// Stops the service
    async fn stop(&mut self) -> Result<(), anyhow::Error> {
        println!("Stopping Event Handler Service");
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Stopped;
        
        // Note: Subscriptions registered through context.subscribe are
        // automatically cleaned up when the service is removed from the registry
        
        Ok(())
    }
    
    /// Handles incoming requests to the service
    async fn handle_request(&self, request: ServiceRequest) -> Result<ServiceResponse, anyhow::Error> {
        println!("Handling request: {}", request.operation);
        
        // ✅ CORRECT: Match on the operation and delegate to specific handlers
        match request.operation.as_str() {
            "store_event" => {
                // Extract the event data from the request
                if let Some(data) = request.params {
                    // Process the event
                    self.process_event(data).await?;
                    
                    // Return success response
                    Ok(ServiceResponse {
                        status: ResponseStatus::Success,
                        message: "Event stored successfully".to_string(),
                        data: Some(ValueType::String("OK".to_string())),
                    })
                } else {
                    // Return error if no data provided
                    Ok(ServiceResponse {
                        status: ResponseStatus::Error,
                        message: "No event data provided".to_string(),
                        data: None,
                    })
                }
            },
            "get_events" => {
                // Get a reference to stored events
                let events = self.events.lock().unwrap();
                
                // Convert events to ValueType for response
                let mut events_array = Vec::new();
                for event in events.iter() {
                    // Create a map for each event
                    let mut event_map = std::collections::HashMap::new();
                    event_map.insert("id".to_string(), ValueType::String(event.id.clone()));
                    event_map.insert("type".to_string(), ValueType::String(event.event_type.clone()));
                    
                    // Add timestamp as unix epoch
                    let timestamp = event.timestamp
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    event_map.insert("timestamp".to_string(), ValueType::Number(timestamp as f64));
                    
                    // Add data if available
                    if let Some(ref data) = event.data {
                        event_map.insert("data".to_string(), data.clone());
                    }
                    
                    events_array.push(ValueType::Map(event_map));
                }
                
                // Create response data
                let mut response_data = std::collections::HashMap::new();
                response_data.insert("events".to_string(), ValueType::Array(events_array));
                response_data.insert("count".to_string(), ValueType::Number(events.len() as f64));
                
                // Return success response with events data
                Ok(ServiceResponse {
                    status: ResponseStatus::Success,
                    message: "Events retrieved successfully".to_string(),
                    data: Some(ValueType::Map(response_data)),
                })
            },
            _ => {
                // Return error for unknown operation
                Ok(ServiceResponse {
                    status: ResponseStatus::Error,
                    message: format!("Unknown operation: {}", request.operation),
                    data: None,
                })
            }
        }
    }
} 