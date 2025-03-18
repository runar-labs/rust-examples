/**
 * Example file demonstrating the service and action macros in KAGI.
 * 
 * This file shows how to create services with the #[service] macro and
 * define action handlers with the #[action] macro.
 */

use anyhow::Result;
use kagi_macros::{action, service, subscribe};
use kagi_node::services::{
    AbstractService, RequestContext, ServiceResponse, ValueType, ResponseStatus
};
use kagi_node::vmap;
use kagi_utils::{vmap_extract_string, vmap_extract_i32, vmap_extract_f64, vmap_extract_bool};
use std::sync::Arc;
use async_trait;

/// Example service that performs data processing operations
/// 
/// The service macro generates implementations of the ServiceInfo trait and AbstractService trait.
/// Parameters:
/// - name: The display name of the service (optional, defaults to struct name in snake_case)
/// - path: The routing path for this service (optional, defaults to name)
/// - description: Human-readable description (optional)
/// - version: Version string (optional, defaults to "0.1.0")
#[service(
    name = "data",
    // path = "data", ommited on purpose to desmostrate that in this csae will uys DataProcessorService.to_snake_case()
    description = "Processes and transforms data",
    version = "1.0.0"
)]
pub struct DataProcessorService {
    // Your service state goes here
    counter: i32,
    // Store received events to demonstrate multiple handlers for the same event
    events_received: Vec<String>,
}

impl DataProcessorService {
    /// Create a new instance of the service
    pub fn new() -> Self {
        Self {
            counter: 0,
            events_received: Vec::new(),
        }
    }
    
    /// The action macro defines methods as action handlers that can be invoked via the node API.
    /// The name parameter specifies the operation name that will be used by the node.request API.
    /// 
    /// Parameters are automatically extracted from the request.
    /// This example shows the Context + Parameters pattern.
    #[action(name = "transform")]
    async fn transform(&self, context: &RequestContext, data: &str) -> Result<String> {
        println!("Processing data: {}", data);
        
        // Create transformed data
        let transformed = data.to_uppercase();
        
        // Publish the transformed data as an event
        let event_data = vmap! {
            "source" => "transform",
            "data" => transformed.clone()
        };  
      
        // Use context to publish the event
        context.publish("events/data_events", event_data).await?;   

        // The action macro handles wrapping the return value in a ServiceResponse
        // Simply return the data or an error
        Ok(transformed)
    }
    
    /// Another action method that increments the counter
    /// 
    /// This example doesn't use the context parameter and only accesses service state.
    #[action(name = "increment")]
    async fn increment(&mut self, value: i32) -> Result<i32> {
        // Update counter by adding the value and 1
        self.counter += value + 1;

        // Publish an event about the counter increment
        let event_data = vmap! {
            "source" => "increment",
            "counter" => self.counter,
            "added_value" => value
        };

        // Return the new counter value
        Ok(self.counter)
    }
    
    /// An action that demonstrates handling named parameters
    /// 
    /// The parameters will be extracted from the request.params map.
    #[action(name = "combine")]
    async fn combine(&self, context: &RequestContext, first: &str, second: &str) -> Result<String> {
        let combined = format!("{} {}", first, second);
        
        // Publish the combined data as an event
        let event_data = vmap! {
            "source" => "combine",
            "data" => combined.clone()
        };
        
        // Use context to publish the event
        context.publish("events/data_events", event_data).await?;   

        // Simply return the combined string
        Ok(combined)
    }

    /// Subscribe to a topic using the full path 
    /// This demonstrates subscribing to a event of another service, it wil lalso show that all handler will receive the same event
    #[subscribe(topic = "events/math_events")]
    async fn on_math_events(&mut self, payload: ValueType) -> Result<()> {
        // Use vmap_extract to get parameters with defaults
        let data = vmap_extract_string!(payload, "data", String::new());
        if !data.is_empty() {
            println!("Received math event: {}", data);
            self.events_received.push(data);// add a lkist ogf event ot his serv8ice also.. to it can also be verified that reveid the event
        }
        Ok(())
    }
}

/// Example service that subscribes to events
/// 
/// This service demonstrates using the #[subscribe] macro to handle events.
#[service(
    name = "EventHandlerService",
    path = "events", // For registry purposes, this path is what matters
    description = "Handles various system events",
    version = "1.0.0"
)]
struct EventHandlerService {
    events_received: Vec<String>,
}

impl EventHandlerService {
    pub fn new() -> Self {
        Self {
            events_received: Vec::new(),
        }
    }
    
    /// Subscribe to a specific topic
    /// 
    /// The #[subscribe] macro will register this method to receive events for the given topic -
    /// in this case the full path is events/text_events. <service>/<event_name>
    #[subscribe(topic = "text_events")]
    async fn handle_text_events(&mut self, payload: ValueType) -> Result<()> {
        // Use vmap_extract to get parameters with defaults
        let data = vmap_extract_string!(payload, "data", String::new());
        if !data.is_empty() {
            println!("Received text event: {}", data);
            self.events_received.push(data);
        }
        Ok(())
    }

    /// Subscribe to a topic using the full path
    /// This demonstrates using the full path including the service name
    #[subscribe(topic = "events/math_events")]
    async fn handle_math_events(&mut self, payload: ValueType) -> Result<()> {
        // Use vmap_extract to get parameters with defaults
        let data = vmap_extract_string!(payload, "data", String::new());
        if !data.is_empty() {
            println!("Received math event: {}", data);
            self.events_received.push(data);
        }
        Ok(())
    }

    /// Handle custom events published directly via the node API
    #[subscribe]
    async fn custom(&mut self, payload: ValueType) -> Result<()> {
        // Use vmap_extract to extract parameters with defaults
        let message = vmap_extract_string!(payload, "message", "no message");
        let timestamp = vmap_extract_string!(payload, "timestamp", "no timestamp");
        let data = vmap_extract_string!(payload, "data", "no data");
        
        let event_text = format!("Custom event: {} at {} with data: {}", message, timestamp, data);
        println!("{}", event_text);
        self.events_received.push(event_text);
        
        Ok(())
    }
    
    /// Action to get the list of received events
    #[action]
    async fn get_events(&self, _context: &RequestContext) -> Result<Vec<ValueType>> {
        let events = self.events_received.iter()
            .map(|e| ValueType::String(e.clone()))
            .collect::<Vec<ValueType>>();
            
        // Simply return the events array
        Ok(events)
    }
}

// For subscription handlers, we need to implement Clone
impl Clone for EventHandlerService {
    fn clone(&self) -> Self {
        Self {
            events_received: self.events_received.clone(),
        }
    }
}

// For subscription handlers, we need to implement Clone
impl Clone for DataProcessorService {
    fn clone(&self) -> Self {
        Self {
            counter: self.counter,
            events_received: self.events_received.clone(),
        }
    }
}

/// Example of using the services with the Node API
#[tokio::main]
async fn main() -> Result<()> {
    // Main function
    println!("KAGI Macros Example");
    println!("==================\n");
    println!("Demonstrating service and action macros\n");

    // Create service instances directly
    let mut data_processor = DataProcessorService::new();
    let mut event_handler = EventHandlerService::new();
    
    println!("Testing service operations directly:");
    
    // Create a request context for testing
    let request_context = RequestContext {
        path: "test/service".to_string(),
        data: ValueType::Null,
        node_handler: Arc::new(DummyNodeHandler {}),
    };
    
    // Test the transform operation directly
    let transform_result = data_processor.transform(&request_context, "Hello, World!").await?;
    println!("1. Transform result: {}", transform_result);
    
    // Test the increment operation directly
    let increment_result = data_processor.increment(5).await?;
    println!("2. Increment result: {}", increment_result);
    
    // Test combining two strings
    let combine_result = data_processor.combine(&request_context, "Hello", "World").await?;
    println!("3. Combine result: {}", combine_result);
    
    // Test event handling
    let event_payload = ValueType::from("Test event payload");
    let handle_event_result = event_handler.handle_text_events(event_payload.clone()).await;
    println!("4. Handle text event result: {:?}", handle_event_result);
    
    // Test custom event handling
    let custom_result = event_handler.custom(event_payload.clone()).await;
    println!("5. Custom event handler result: {:?}", custom_result);
    
    // Get events
    let get_events_result = event_handler.get_events(&request_context).await?;
    println!("6. Get events result: {:?}", get_events_result);
    
    println!("\nAll operations completed successfully!");
    
    Ok(())
}

// Replace the DummyNodeHandler implementation with a working one
#[derive(Clone)]
struct DummyNodeHandler {}

#[async_trait::async_trait]
impl kagi_node::services::NodeRequestHandler for DummyNodeHandler {
    async fn request(&self, path: String, params: ValueType) -> Result<kagi_node::services::ServiceResponse> {
        Ok(kagi_node::services::ServiceResponse {
            status: kagi_node::services::ResponseStatus::Success,
            message: "OK".to_string(),
            data: Some(ValueType::Null),
        })
    }

    async fn publish(&self, _topic: String, _data: ValueType) -> Result<()> {
        Ok(())
    }

    async fn subscribe(
        &self,
        _topic: String,
        _handler: Box<dyn Fn(ValueType) -> Result<()> + Send + Sync>,
    ) -> Result<String> {
        Ok("subscription-id".to_string())
    }

    async fn subscribe_with_options(
        &self,
        _topic: String,
        _handler: Box<dyn Fn(ValueType) -> Result<()> + Send + Sync>,
        _options: kagi_node::services::SubscriptionOptions,
    ) -> Result<String> {
        Ok("subscription-id-with-options".to_string())
    }

    async fn unsubscribe(&self, _topic: String, _subscription_id: Option<&str>) -> Result<()> {
        Ok(())
    }
}
