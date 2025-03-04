# Model Context Protocol (MCP) Client Implementation in Rust

This document outlines a comprehensive plan for implementing a Model Context Protocol (MCP) client in Rust, with a focus on tools integration for LLM agents.

## 1. Project Overview

### 1.1 Objective
Implement a Rust client for the Model Context Protocol that enables LLM agents to discover and use tools provided by MCP servers.

### 1.2 Key Features
- MCP client connection management
- Tool discovery and listing
- Tool invocation and response handling
- Error handling and recovery
- Integration with existing LLM agent architecture

## 2. Architecture

### 2.1 High-Level Components
```
┌──────────────────┐     ┌───────────────────┐     ┌───────────────────┐
│                  │     │                   │     │                   │
│   LLM Agent      ├────►│   MCP Client      ├────►│   MCP Servers     │
│                  │     │                   │     │                   │
└──────────────────┘     └───────────────────┘     └───────────────────┘
```

### 2.2 Core Components
1. **Transport Layer**: Manages WebSocket connections to MCP servers
2. **Message Layer**: Handles JSON-RPC message serialization/deserialization
3. **Client Core**: Implements the MCP client protocol
4. **Tools Interface**: Provides a clean API for tool discovery and usage
5. **Agent Integration**: Bridges the MCP client with the LLM agent

## 3. Detailed Implementation Plan

### 3.1 Project Setup
```rust
// Cargo.toml
[package]
name = "mcp-client"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.28", features = ["full"] }
tokio-tungstenite = "0.19"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
futures = "0.3"
async-trait = "0.1"
tracing = "0.1"
url = "2.3"
```

### 3.2 Core Data Structures

#### 3.2.1 Message Types
```rust
// src/protocol/messages.rs

/// Base JSON-RPC 2.0 message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,  // Always "2.0"
    pub id: Option<serde_json::Value>,
    #[serde(flatten)]
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Request(Request),
    Response(Response),
    Notification(Notification),
    Error(ErrorResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: JsonRpcError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
```

#### 3.2.2 Tool-Related Structures
```rust
// src/protocol/tools.rs

/// Represents a tool provided by an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema for tool input
    pub output_schema: Option<serde_json::Value>,  // JSON Schema for tool output
    #[serde(rename = "_meta")]
    pub meta: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for a ListToolsRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Response from a ListToolsRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

/// Parameters for a CallToolRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub id: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Response from a CallToolRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub output: serde_json::Value,
}
```

### 3.3 Client Implementation

```rust
// src/client.rs

pub struct McpClient {
    connection: Option<WebSocketConnection>,
    request_id: AtomicUsize,
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Result<serde_json::Value, McpError>>>>>,
}

impl McpClient {
    pub async fn connect(url: &str) -> Result<Self, McpError> {
        // Implementation details for establishing connection
    }

    pub async fn initialize(&self, client_info: ClientInfo) -> Result<InitializeResult, McpError> {
        // Send initialize request and handle response
    }

    pub async fn list_tools(&self) -> Result<Vec<Tool>, McpError> {
        let params = ListToolsParams { categories: None };
        let result = self.send_request("listTools", &params).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn call_tool(&self, tool_id: &str, input: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let params = CallToolParams {
            id: tool_id.to_string(),
            input,
            stream: None,
        };
        
        let result = self.send_request("callTool", &params).await?;
        let tool_result: CallToolResult = serde_json::from_value(result)?;
        Ok(tool_result.output)
    }

    async fn send_request<P: Serialize>(&self, method: &str, params: &P) -> Result<serde_json::Value, McpError> {
        // Implementation for sending requests and waiting for responses
    }
}
```

### 3.4 Agent Integration Layer

```rust
// src/agent/integration.rs

pub struct McpToolProvider {
    client: Arc<McpClient>,
    tool_cache: RwLock<HashMap<String, Tool>>,
}

impl McpToolProvider {
    pub async fn new(server_url: &str) -> Result<Self, McpError> {
        let client = McpClient::connect(server_url).await?;
        let client = Arc::new(client);
        
        // Initialize the client
        client.initialize(ClientInfo {
            name: "rust-llm-agent".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }).await?;
        
        Ok(Self {
            client,
            tool_cache: RwLock::new(HashMap::new()),
        })
    }
    
    pub async fn refresh_tools(&self) -> Result<(), McpError> {
        let tools = self.client.list_tools().await?;
        let mut cache = self.tool_cache.write().await;
        *cache = tools.into_iter().map(|t| (t.id.clone(), t)).collect();
        Ok(())
    }
    
    pub async fn execute_tool(&self, tool_id: &str, input: serde_json::Value) -> Result<serde_json::Value, McpError> {
        self.client.call_tool(tool_id, input).await
    }
}

#[async_trait]
impl ToolProvider for McpToolProvider {
    async fn list_tools(&self) -> Vec<AgentTool> {
        // Convert MCP tools to agent's internal tool representation
        let cache = self.tool_cache.read().await;
        cache.values()
            .map(|tool| AgentTool {
                name: tool.id.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            })
            .collect()
    }
    
    async fn execute(&self, tool_name: &str, parameters: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        self.execute_tool(tool_name, parameters)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
    }
}
```

## 4. JSON Format Examples

### 4.1 Initialize Request

```json
{
  "id": 1,
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "roots": {
        "listChanged": true
      }
    },
    "clientInfo": {
      "name": "rust-llm-agent",
      "version": "0.1.0"
    }
  }
}
```

### 4.2 List Tools Request

```json
{
  "id": 2,
  "jsonrpc": "2.0",
  "method": "listTools",
  "params": {}
}
```

### 4.3 List Tools Response

```json
{
  "id": 2,
  "jsonrpc": "2.0",
  "result": {
    "tools": [
      {
        "id": "search",
        "name": "Search",
        "description": "Search the web for information",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": {
              "type": "string",
              "description": "The search query"
            },
            "limit": {
              "type": "integer",
              "description": "Maximum number of results to return",
              "default": 5
            }
          },
          "required": ["query"]
        }
      },
      {
        "id": "database",
        "name": "Database Query",
        "description": "Execute a query against the database",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": {
              "type": "string",
              "description": "SQL query to execute"
            }
          },
          "required": ["query"]
        }
      }
    ]
  }
}
```

### 4.4 Call Tool Request

```json
{
  "id": 3,
  "jsonrpc": "2.0",
  "method": "callTool",
  "params": {
    "id": "search",
    "input": {
      "query": "latest Rust language features",
      "limit": 3
    }
  }
}
```

### 4.5 Call Tool Response

```json
{
  "id": 3,
  "jsonrpc": "2.0",
  "result": {
    "output": {
      "results": [
        {
          "title": "What's New in Rust 1.74.0",
          "url": "https://blog.rust-lang.org/2023/11/16/Rust-1.74.0.html",
          "snippet": "Rust 1.74.0 introduces new features including let-else statements and GATs stabilization..."
        },
        {
          "title": "Rust 2024 Edition Guide",
          "url": "https://doc.rust-lang.org/edition-guide/rust-2024/index.html",
          "snippet": "The Rust 2024 edition includes several new language features and improvements..."
        },
        {
          "title": "Async fn in traits is now stable",
          "url": "https://blog.rust-lang.org/2023/12/21/async-fn-in-trait-stabilization.html",
          "snippet": "After years of development, async functions in traits are now stable in Rust..."
        }
      ],
      "totalResults": 125
    }
  }
}
```

## 5. Implementation Timeline

### Phase 1: Core Infrastructure (Weeks 1-2)
- Set up project structure
- Implement transport layer
- Implement message serialization/deserialization
- Create basic client with connection handling

### Phase 2: MCP Protocol Implementation (Weeks 3-4)
- Implement initialization flow
- Add tool discovery capabilities
- Implement tool invocation
- Add error handling

### Phase 3: Agent Integration (Weeks 5-6)
- Design tool provider interface
- Implement MCP tool provider
- Create tool result processing
- Add caching and performance optimizations

### Phase 4: Testing and Documentation (Weeks 7-8)
- Create comprehensive test suite
- Add documentation
- Create example implementations
- Performance testing and optimization

## 6. Testing Strategy

### 6.1 Unit Tests
- Test message serialization/deserialization
- Test connection handling
- Test error recovery

### 6.2 Integration Tests
- Test against mock MCP servers
- Test full initialization flow
- Test tool discovery and invocation

### 6.3 End-to-End Tests
- Test with real-world MCP servers
- Test integration with the LLM agent
- Test error scenarios and recovery

## 7. Conclusion

This implementation plan provides a roadmap for building a robust MCP client in Rust, with a focus on tool integration for LLM agents. By following this plan, we can create a modular, performant, and reliable implementation that seamlessly integrates with our existing agent architecture.