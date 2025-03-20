/**
 * Example file demonstrating service implementation and Node API usage in Runar.
 * 
 * This file shows how to:
 * 1. Create services that implement AbstractService
 * 2. Define action handlers for operations
 * 3. Register event handlers for subscriptions
 * 4. Interact with services through the Node API
 */

use anyhow::Result;
use runar_node::{
    services::{
        AbstractService, RequestContext, ResponseStatus, ServiceResponse, 
        ServiceState, ServiceMetadata, ValueType, ServiceRequest
    }
};
use std::sync::{Arc, Mutex};
use tokio;
use std::collections::HashMap;
use async_trait::async_trait;

/// Example of a data processing service
struct DataProcessorService {
    name: String,
    path: String,
    description: String,
    version: String,
    state: Arc<Mutex<ServiceState>>,
    counter: Arc<Mutex<i32>>,
}

impl Clone for DataProcessorService {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            path: self.path.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            state: Arc::clone(&self.state),
            counter: Arc::clone(&self.counter),
        }
    }
}

impl DataProcessorService {
    /// Create a new instance of the service
    pub fn new() -> Self {
        Self {
            name: "data".to_string(),
            path: "/services/data".to_string(),
            description: "A service for processing data operations".to_string(),
            version: "1.0.0".to_string(),
            state: Arc::new(Mutex::new(ServiceState::Created)),
            counter: Arc::new(Mutex::new(0)),
        }
    }
    
    /// Transform a string to uppercase
    async fn transform_string(&self, ctx: &RequestContext) -> Result<ServiceResponse> {
        let input = match ctx.data.get("input") {
            Some(ValueType::String(s)) => s.clone(),
            _ => return Ok(ServiceResponse::error("Missing input parameter".to_string())),
        };

        let result = format!("Transformed: {}", input.to_uppercase());
        
        // Publish an event that our transformation occurred
        let mut event_data = HashMap::new();
        event_data.insert("text".to_string(), ValueType::String(result.clone()));
        ctx.publish("text_event", ValueType::Map(event_data)).await?;
        
        // Create a map with the result for the response
        let mut result_map = HashMap::new();
        result_map.insert("result".to_string(), ValueType::String(result));
        
        Ok(ServiceResponse::success(
            "String transformed successfully".to_string(),
            Some(ValueType::Map(result_map)),
        ))
    }
    
    /// Increment the counter by the specified value
    async fn increment_counter(&self, ctx: &RequestContext) -> Result<ServiceResponse> {
        // Increment counter and get its value without holding the lock across await points
        let value = {
            let mut counter = self.counter.lock().unwrap();
            *counter += 1;
            *counter
        };
        
        // Publish an event about the counter value
        let mut event_data = HashMap::new();
        event_data.insert("value".to_string(), ValueType::Number(value as f64));
        ctx.publish("math_event", ValueType::Map(event_data)).await?;
        
        // Create a map with the counter for the response
        let mut result_map = HashMap::new();
        result_map.insert("counter".to_string(), ValueType::Number(value as f64));
        
        Ok(ServiceResponse::success(
            "Counter incremented successfully".to_string(),
            Some(ValueType::Map(result_map)),
        ))
    }
    
    /// Combine two strings
    async fn combine_strings(&self, ctx: &RequestContext) -> Result<ServiceResponse> {
        let str1 = match ctx.data.get("str1") {
            Some(ValueType::String(s)) => s.clone(),
            _ => return Ok(ServiceResponse::error("Missing str1 parameter".to_string())),
        };

        let str2 = match ctx.data.get("str2") {
            Some(ValueType::String(s)) => s.clone(),
            _ => return Ok(ServiceResponse::error("Missing str2 parameter".to_string())),
        };

        let result = format!("Combined: {} + {}", str1, str2);
        
        // Publish a custom event
        ctx.publish("custom_event", ValueType::String(result.clone())).await?;
        
        // Create a map with the result for the response
        let mut result_map = HashMap::new();
        result_map.insert("result".to_string(), ValueType::String(result));
        
        Ok(ServiceResponse::success(
            "Strings combined successfully".to_string(),
            Some(ValueType::Map(result_map)),
        ))
    }
}

#[async_trait]
impl AbstractService for DataProcessorService {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn path(&self) -> &str {
        &self.path
    }
    
    fn state(&self) -> ServiceState {
        *self.state.lock().unwrap()
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    fn metadata(&self) -> ServiceMetadata {
        ServiceMetadata {
            name: self.name.clone(),
            path: self.path.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            state: self.state(),
            operations: vec!["transform".to_string(), "increment".to_string(), "combine".to_string()],
        }
    }
    
    // Method signatures must match exactly with AbstractService trait
    async fn init(&mut self, _ctx: &RequestContext) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Initialized;
        Ok(())
    }
    
    async fn start(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Running;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Stopped;
        Ok(())
    }
    
    async fn handle_request(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        // Print request for debugging
        println!("Data Processor received request: operation={}, params={:?}", 
                 request.operation, request.params);
        
        // Extract parameters from the request
        let data_map = match &request.request_context.data {
            ValueType::Map(map) => {
                let mut new_map = map.clone();
                
                // Add parameters to the map if they exist
                if let Some(ValueType::Map(param_map)) = &request.params {
                    for (key, value) in param_map {
                        new_map.insert(key.clone(), value.clone());
                    }
                }
                
                new_map
            },
            _ => {
                let mut new_map = HashMap::new();
                
                // Add parameters to the map if they exist
                if let Some(ValueType::Map(param_map)) = &request.params {
                    for (key, value) in param_map {
                        new_map.insert(key.clone(), value.clone());
                    }
                }
                
                new_map
            }
        };
        
        // Create a new context with the updated data map
        let new_context = RequestContext::new(
            request.request_context.path.clone(),
            ValueType::Map(data_map),
            request.request_context.node_handler.clone()
        );
        
        match request.operation.as_str() {
            "transform" => self.transform_string(&new_context).await,
            "increment" => self.increment_counter(&new_context).await,
            "combine" => self.combine_strings(&new_context).await,
            _ => Ok(ServiceResponse::error(format!("Unknown operation: {}", request.operation)))
        }
    }
}

/// Example of an event handler service
struct EventHandlerService {
    name: String,
    path: String,
    description: String,
    version: String,
    state: Arc<Mutex<ServiceState>>,
    received_events: Arc<Mutex<Vec<String>>>,
}

impl Clone for EventHandlerService {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            path: self.path.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            state: Arc::clone(&self.state),
            received_events: Arc::clone(&self.received_events),
        }
    }
}

impl EventHandlerService {
    /// Create a new instance of the service
    pub fn new() -> Self {
        Self {
            name: "events".to_string(),
            path: "/services/events".to_string(),
            description: "A service for handling various events".to_string(),
            version: "1.0.0".to_string(),
            state: Arc::new(Mutex::new(ServiceState::Created)),
            received_events: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    async fn handle_text_event(&self, data: ValueType) -> Result<()> {
        if let ValueType::Map(map) = data {
            if let Some(ValueType::String(text)) = map.get("text") {
                let event_text = format!("Received text event: {}", text);
                self.received_events.lock().unwrap().push(event_text);
            }
        }
        
        Ok(())
    }
    
    async fn handle_math_event(&self, data: ValueType) -> Result<()> {
        if let ValueType::Map(map) = data {
            if let Some(ValueType::Number(value)) = map.get("value") {
                let event_text = format!("Received math event with value: {}", value);
                self.received_events.lock().unwrap().push(event_text);
            }
        }
        
        Ok(())
    }
    
    async fn handle_custom_event(&self, data: ValueType) -> Result<()> {
        let event_text = format!("Received custom event: {:?}", data);
        self.received_events.lock().unwrap().push(event_text);
        
        Ok(())
    }
    
    /// Action to retrieve received events
    async fn get_events(&self, _ctx: &RequestContext) -> Result<ServiceResponse> {
        // Get events without holding the lock across await points
        let events = self.received_events.lock().unwrap().clone();
        
        let events_value: Vec<ValueType> = events
            .into_iter()
            .map(ValueType::String)
            .collect();
        
        // Create a map with the events for the response
        let mut result_map = HashMap::new();
        result_map.insert("events".to_string(), ValueType::Array(events_value));
        
        Ok(ServiceResponse::success(
            "Events retrieved successfully".to_string(),
            Some(ValueType::Map(result_map)),
        ))
    }
}

#[async_trait]
impl AbstractService for EventHandlerService {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn path(&self) -> &str {
        &self.path
    }
    
    fn state(&self) -> ServiceState {
        *self.state.lock().unwrap()
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    fn metadata(&self) -> ServiceMetadata {
        ServiceMetadata {
            name: self.name.clone(),
            path: self.path.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            state: self.state(),
            operations: vec!["get_events".to_string()],
        }
    }
    
    // Method signatures must match exactly with AbstractService trait
    async fn init(&mut self, ctx: &RequestContext) -> Result<()> {
        // Set state without holding lock across await points
        {
            let mut state = self.state.lock().unwrap();
            *state = ServiceState::Initialized;
        }
        
        // Create references to self for event handlers
        let this = Arc::new(self.clone());
        
        // Subscribe to events using closures
        let this_clone = Arc::clone(&this);
        ctx.subscribe("text_event", move |data| {
            let this = this_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = this.handle_text_event(data).await {
                    eprintln!("Error handling text event: {}", e);
                }
            });
            Ok(())
        }).await?;
        
        let this_clone = Arc::clone(&this);
        ctx.subscribe("math_event", move |data| {
            let this = this_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = this.handle_math_event(data).await {
                    eprintln!("Error handling math event: {}", e);
                }
            });
            Ok(())
        }).await?;
        
        let this_clone = Arc::clone(&this);
        ctx.subscribe("custom_event", move |data| {
            let this = this_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = this.handle_custom_event(data).await {
                    eprintln!("Error handling custom event: {}", e);
                }
            });
            Ok(())
        }).await?;
        
        Ok(())
    }
    
    async fn start(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Running;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        *state = ServiceState::Stopped;
        Ok(())
    }
    
    async fn handle_request(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        // Print request for debugging
        println!("Event Handler received request: operation={}, params={:?}", 
                 request.operation, request.params);
        
        // Extract parameters from the request
        let data_map = match &request.request_context.data {
            ValueType::Map(map) => {
                let mut new_map = map.clone();
                
                // Add parameters to the map if they exist
                if let Some(ValueType::Map(param_map)) = &request.params {
                    for (key, value) in param_map {
                        new_map.insert(key.clone(), value.clone());
                    }
                }
                
                new_map
            },
            _ => {
                let mut new_map = HashMap::new();
                
                // Add parameters to the map if they exist
                if let Some(ValueType::Map(param_map)) = &request.params {
                    for (key, value) in param_map {
                        new_map.insert(key.clone(), value.clone());
                    }
                }
                
                new_map
            }
        };
        
        // Create a new context with the updated data map
        let new_context = RequestContext::new(
            request.request_context.path.clone(),
            ValueType::Map(data_map),
            request.request_context.node_handler.clone()
        );
        
        match request.operation.as_str() {
            "get_events" => self.get_events(&new_context).await,
            _ => Ok(ServiceResponse::error(format!("Unknown operation: {}", request.operation)))
        }
    }
}

/// Example of using the services with the Node API
#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Macros Node Example...");
    
    // Create a temporary directory for node data
    let temp_dir = std::env::temp_dir().join("runar_node_example");
    let _ = std::fs::create_dir_all(&temp_dir);
    let db_path = temp_dir.join("node.db");

    // Create node config with only the fields it actually has
    let node_config = runar_node::NodeConfig {
        network_id: "example".to_string(),
        node_id: Some("example-node".to_string()),
        node_path: temp_dir.to_string_lossy().to_string(),
        db_path: db_path.to_string_lossy().to_string(),
        p2p_config: None,
        state_path: None,
        test_network_ids: None,
        bootstrap_nodes: None,
        listen_addr: None,
    };
    
    // Create the node
    let mut node = runar_node::node::Node::new(node_config).await?;
    
    // Initialize the node
    node.init().await?;
    
    // Create our services
    let mut data_service = DataProcessorService::new();
    let mut event_service = EventHandlerService::new();
    
    // Initialize services
    let context = node.create_request_context("init").await?;
    
    data_service.init(&context).await?;
    event_service.init(&context).await?;
    
    // Register services with the node using the proper add_service method
    node.add_service(data_service).await?;
    node.add_service(event_service).await?;
    
    // Start the services
    node.start_services().await?;
    
    println!("Services initialized and started");
    
    // Test operations
    
    // 1. Transform a string
    println!("\nTesting string transformation:");
    let transform_result = node.request(
        "data/transform".to_string(),
        ValueType::Map({
            let mut map = HashMap::new();
            map.insert("input".to_string(), ValueType::String("hello world".to_string()));
            map
        })
    ).await?;
    
    if transform_result.status == ResponseStatus::Success {
        if let Some(data) = &transform_result.data {
            if let ValueType::Map(map) = data {
                if let Some(ValueType::String(s)) = map.get("result") {
                    println!("Transform result: {}", s);
                } else {
                    println!("No result found in response");
                }
            } else {
                println!("Unexpected response format");
            }
        } else {
            println!("No data in response");
        }
    } else {
        println!("Error: {}", transform_result.message);
    }
    
    // 2. Combine strings
    println!("\nTesting string combination:");
    let combine_result = node.request(
        "data/combine".to_string(),
        ValueType::Map({
            let mut map = HashMap::new();
            map.insert("str1".to_string(), ValueType::String("Hello".to_string()));
            map.insert("str2".to_string(), ValueType::String("Runar World!".to_string()));
            map
        })
    ).await?;
    
    if combine_result.status == ResponseStatus::Success {
        if let Some(data) = &combine_result.data {
            if let ValueType::Map(map) = data {
                if let Some(ValueType::String(s)) = map.get("result") {
                    println!("Combine result: {}", s);
                } else {
                    println!("No result found in response");
                }
            } else {
                println!("Unexpected response format");
            }
        } else {
            println!("No data in response");
        }
    } else {
        println!("Error: {}", combine_result.message);
    }
    
    // 3. Increment counter
    println!("\nTesting counter increment:");
    let increment_result = node.request(
        "data/increment".to_string(),
        ValueType::Map(HashMap::new())
    ).await?;
    
    if increment_result.status == ResponseStatus::Success {
        if let Some(data) = &increment_result.data {
            if let ValueType::Map(map) = data {
                if let Some(ValueType::Number(n)) = map.get("counter") {
                    println!("Increment result: Counter = {}", n);
                } else {
                    println!("No counter value found in response");
                }
            } else {
                println!("Unexpected response format");
            }
        } else {
            println!("No data in response");
        }
    } else {
        println!("Error: {}", increment_result.message);
    }
    
    // 4. Get received events
    println!("\nGetting received events:");
    // Sleep to ensure events are processed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let events_result = node.request(
        "events/get_events".to_string(),
        ValueType::Map(HashMap::new())
    ).await?;
    
    if events_result.status == ResponseStatus::Success {
        if let Some(data) = &events_result.data {
            if let ValueType::Map(map) = data {
                if let Some(ValueType::Array(events)) = map.get("events") {
                    println!("Received {} events:", events.len());
                    for (i, event) in events.iter().enumerate() {
                        if let ValueType::String(s) = event {
                            println!("  {}: {}", i+1, s);
                        } else {
                            println!("  {}: Invalid event", i+1);
                        }
                    }
                } else {
                    println!("No events field in response");
                }
            } else {
                println!("Unexpected response format");
            }
        } else {
            println!("No data in response");
        }
    } else {
        println!("Error: {}", events_result.message);
    }
    
    // Shutdown node
    println!("\nShutting down node...");
    node.stop().await?;
    
    println!("Example completed successfully!");
    
    Ok(())
} 