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
    // path = "data", ommited on purpose to desmostrate that in this csae will uyse name as path  
    description = "Processes and transforms data",
    version = "1.0.0"
)]
struct DataProcessorService {
    // Your service state goes here
    counter: u32,
}

impl DataProcessorService {
    /// Create a new instance of the service
    pub fn new() -> Self {
        Self {
            counter: 0,
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
    async fn increment_counter(&mut self, context: &RequestContext, value: u32) -> Result<u32> {
        // Add the passed value to the counter
        self.counter += value + 1;

        // Publish the counter value as an event
        let event_data = vmap! {
            "source" => "increment",
            "counter" => self.counter,
            "added_value" => value
        };
        
        // Use context to publish the event
        context.publish("events/data_events", event_data).await?;
        
        // Simply return the counter value
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
    // #[subscribe(topic = "events/text_events")]  the example above shows a shorthand way omiting the service path, this line show using teh full path. both need to work
    async fn handle_text_events(&mut self, payload: ValueType) -> Result<()> {
        // Use vmap! to get parameters with defaults
        let data = vmap!(payload, "data" => String::new());
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
        // Use vmap! to get parameters with defaults
        let data = vmap!(payload, "data" => String::new());
        if !data.is_empty() {
            println!("Received math event: {}", data);
            self.events_received.push(data);
        }
        Ok(())
    }

    /// Handle custom events published directly via the node API
    // #[subscribe(topic = "events/custom")]
    #[subscribe]
    async fn custom(&mut self, payload: ValueType) -> Result<()> {
        // Use vmap! to extract parameters with defaults
        let message = vmap!(payload, "message" => "no message");
        let timestamp = vmap!(payload, "timestamp" => "no timestamp");
        let data = vmap!(payload, "data" => "no data");
        
        let event_text = format!("Custom event: {} at {} with data: {}", message, timestamp, data);
        println!("{}", event_text);
        self.events_received.push(event_text);
        
        Ok(())
    }
    
    /// Action to get the list of received events
    // #[action(name = "get_events")]
    #[action] //NOTE: this will use the name of the method as the action name
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

/// Example of using the services with the Node API
#[tokio::main]
async fn main() -> Result<()> {
    use kagi_node::node::{Node, NodeConfig};
    use std::time::Duration;
    
    // Create and configure the node
    let config = NodeConfig::new(
        "example_node",
        "./data",
        "./data/db"
    );
    
    // Create a new node instance
    let mut node = Node::new(config).await?;
    
    // Initialize the node
    node.init().await?;
    
    // Create service instances
    let data_processor = DataProcessorService::new();
    let event_handler = EventHandlerService::new();
    
    // Add services to the node
    node.add_service(data_processor).await?;
    node.add_service(event_handler).await?;
    
    // Start the node
    node.start().await?;
    
    // Transform a string using the data service
    let transform_result = node.request(
        "data/transform",
        vmap! {
            "data" => "hello world"
        }
    ).await?;
    
    println!("Transform result: {:?}", transform_result);
    assert_eq!(transform_result.status, ResponseStatus::Success);

    //for actions that receive a single parameter, we shouod be able to send the single parameter
    //without having to send a vmap!  
    let transform_result2 = node.request(
        "data/transform",
         "hello world" 
    ).await?;
    
    println!("Transform result2: {:?}", transform_result2);
    assert_eq!(transform_result.status, ResponseStatus::Success);
    
    // Using vmap! to extract values from the response
    let transformed_data = vmap!(transform_result.data, => String::new());
    assert_eq!(transformed_data, "HELLO WORLD");
    
    // Combine two strings
    let combine_result = node.request(
        "data/combine",
        vmap! {
            "first" => "john",
            "second" => "doe"
        }
    ).await?;
    
    println!("Combine result: {:?}", combine_result);
    assert_eq!(combine_result.status, ResponseStatus::Success);
    
    // Using vmap! to extract values from the response
    let combined_data = vmap!(combine_result.data, => String::new());
    assert_eq!(combined_data, "john doe");
    
    // Increment the counter - using a value parameter
    let increment_result = node.request(
        "data/increment",
        vmap! {
            "value" => 0  // Explicitly send 0 as initial value
        }
    ).await?;
    
    println!("Increment result: {:?}", increment_result);
    assert_eq!(increment_result.status, ResponseStatus::Success);
    
    // Using vmap! to extract values from the response
    let counter_value = vmap!(increment_result.data, => 0.0);
    assert_eq!(counter_value, 1.0);
    
    // Wait a bit for events to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Publish an event directly using the node API
    node.publish(
        "events/custom",
        vmap! {
            "message" => "direct publish",
            "timestamp" => chrono::Utc::now().to_rfc3339(),
            "data" => "custom event data"
        }
    ).await?;
    
    // Allow time for custom event to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Get events received by the event handler
    let events_result = node.request(
        "events/get_events",
        vmap! {}
    ).await?;
    
    println!("Events result: {:?}", events_result);
    assert_eq!(events_result.status, ResponseStatus::Success);
    
    // Get the events array from the response using vmap!
    let events = vmap!(events_result.data, => Vec::<ValueType>::new());
    println!("Received {} events", events.len());
    
    // Print each event for verification
    for (i, event) in events.iter().enumerate() {
        let event_str = vmap!(event, => "<invalid event>");
        println!("Event {}: {}", i + 1, event_str);
    }
    
    // Gracefully shut down the node
    node.stop().await?;
    
    Ok(())
} 