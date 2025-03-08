//! MCP configuration handling

use crate::tools::ToolResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// MCP server configuration structure matching the .termineer/config.json format
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Command to execute for this MCP server
    pub command: String,

    /// Arguments for the command
    pub args: Vec<String>,
    
    /// Environment variables to set for the command
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Complete MCP configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpConfig {
    /// Map of server name to server configuration
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    /// Get path to the home directory config
    fn get_home_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|path| path.join(".termineer").join("mcp").join("config.json"))
    }
    
    /// Get path to the local config
    fn get_local_config_path() -> PathBuf {
        PathBuf::from(".termineer").join("config.json")
    }
    
    /// Merge another config into this one, with this one taking precedence
    pub fn merge(&mut self, other: McpConfig) {
        // Merge mcp_servers maps, with self taking precedence
        for (server_name, server_config) in other.mcp_servers {
            if !self.mcp_servers.contains_key(&server_name) {
                self.mcp_servers.insert(server_name, server_config);
            }
        }
    }
    
    /// Load MCP configuration from .termineer/config.json and ~/.termineer/mcp/config.json
    pub fn load() -> Result<Option<Self>> {
        let mut result = None;
        
        // Try loading config from home directory first
        if let Some(home_path) = Self::get_home_config_path() {
            if home_path.exists() {
                let config_content = std::fs::read_to_string(&home_path)
                    .with_context(|| format!("Failed to read home config file: {:?}", home_path))?;
                
                let home_config: McpConfig = serde_json::from_str(&config_content)
                    .with_context(|| "Failed to parse home MCP configuration")?;
                
                result = Some(home_config);
            }
        }
        
        // Check for local config
        let local_path = Self::get_local_config_path();
        if local_path.exists() {
            let config_content = std::fs::read_to_string(&local_path)
                .with_context(|| format!("Failed to read local config file: {:?}", local_path))?;
            
            let local_config: McpConfig = serde_json::from_str(&config_content)
                .with_context(|| "Failed to parse local MCP configuration")?;
            
            // If we have a home config, merge the local config into it (local takes precedence)
            if let Some(ref mut combined_config) = result {
                combined_config.merge(local_config);
            } else {
                // Otherwise just use the local config
                result = Some(local_config);
            }
        }
        
        Ok(result)
    }
}

/// Get a list of configured MCP servers for template rendering
pub fn get_server_list() -> Result<Vec<String>> {
    // Load configuration
    match McpConfig::load()? {
        Some(config) => Ok(config.mcp_servers.keys().cloned().collect()),
        None => Ok(vec![]), // No configuration, return empty list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_mcp_config_loading() {
        // Create a temporary test directory
        let temp_dir = PathBuf::from("./target/test_mcp_config");
        let term_dir = temp_dir.join(".termineer");
        fs::create_dir_all(&term_dir).unwrap();

        // Write a test config file
        let config_path = term_dir.join("config.json");
        let test_config = r#"{
          "mcpServers": {
            "test-server": {
              "command": "echo",
              "args": ["MCP server test"],
              "env": {
                "TEST_VAR": "test_value",
                "ANOTHER_VAR": "another_value"
              }
            }
          }
        }"#;
        fs::write(&config_path, test_config).unwrap();

        // Change to the test directory and load the config
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Test both loading the full config and just the server list
        let config = McpConfig::load().unwrap();
        assert!(config.is_some(), "Config should be loaded successfully");
        let config = config.unwrap();
        assert!(
            config.mcp_servers.contains_key("test-server"),
            "Config should contain test-server"
        );

        let server_list = get_server_list().unwrap();
        assert!(!server_list.is_empty(), "Server list should not be empty");
        assert!(
            server_list.contains(&"test-server".to_string()),
            "Server list should contain test-server"
        );

        // Restore the original directory and clean up
        std::env::set_current_dir(original_dir).unwrap();
        fs::remove_dir_all(temp_dir).unwrap();
    }
    
    #[test]
    fn test_config_merge() {
        // Create base config
        let mut base_config = McpConfig {
            mcp_servers: HashMap::new(),
        };
        
        // Add server to base config with environment variables
        let mut env_vars1 = HashMap::new();
        env_vars1.insert("ENV1".to_string(), "value1".to_string());
        env_vars1.insert("COMMON".to_string(), "base_value".to_string());
        
        base_config.mcp_servers.insert(
            "server1".to_string(),
            McpServerConfig {
                command: "cmd1".to_string(),
                args: vec!["arg1".to_string()],
                env: env_vars1,
            },
        );
        
        // Create second config with different server and overlapping server
        let mut other_config = McpConfig {
            mcp_servers: HashMap::new(),
        };
        
        // Add unique server to other config (with empty env map to test default)
        other_config.mcp_servers.insert(
            "server2".to_string(),
            McpServerConfig {
                command: "cmd2".to_string(),
                args: vec!["arg2".to_string()],
                env: HashMap::new(),
            },
        );
        
        // Add overlapping server with different config and env vars
        let mut env_vars2 = HashMap::new();
        env_vars2.insert("ENV2".to_string(), "value2".to_string());
        env_vars2.insert("COMMON".to_string(), "override_value".to_string());
        
        other_config.mcp_servers.insert(
            "server1".to_string(),
            McpServerConfig {
                command: "different".to_string(),
                args: vec!["different-arg".to_string()],
                env: env_vars2,
            },
        );
        
        // Merge configs (base should have priority)
        base_config.merge(other_config);
        
        // Check results
        assert_eq!(base_config.mcp_servers.len(), 2, "Should have 2 servers after merge");
        
        // Check that server1 keeps original values (base has priority)
        let server1 = base_config.mcp_servers.get("server1").unwrap();
        assert_eq!(server1.command, "cmd1", "server1 command should remain from base");
        assert_eq!(server1.args[0], "arg1", "server1 args should remain from base");
        assert_eq!(server1.env.get("ENV1").unwrap(), "value1", "server1 env var ENV1 should remain from base");
        assert_eq!(server1.env.get("COMMON").unwrap(), "base_value", "server1 env var COMMON should remain from base");
        assert!(!server1.env.contains_key("ENV2"), "server1 should not have ENV2 from other config");
        
        // Check that server2 was added
        let server2 = base_config.mcp_servers.get("server2").unwrap();
        assert_eq!(server2.command, "cmd2", "server2 should be added from other");
        assert_eq!(server2.args[0], "arg2", "server2 args should be added from other");
        assert!(server2.env.is_empty(), "server2 env vars should be empty");
    }
}

/// Initialize MCP connections from configuration file
pub async fn initialize_mcp_from_config(silent_mode: bool) -> Result<Vec<ToolResult>> {
    // Load configuration
    let config = match McpConfig::load()? {
        Some(config) => config,
        None => return Ok(vec![]), // No configuration, return empty results
    };

    let mut results = Vec::new();

    // Connect to each configured MCP server
    for (server_name, server_config) in config.mcp_servers {
        // Log the connection attempt
        if !silent_mode {
            bprintln!(
                "🔌 Connecting to MCP server '{}' with command: {} {}",
                server_name,
                server_config.command,
                server_config.args.join(" ")
            );
        }

        // Connect the server using a direct implementation to register with the friendly name
        let result = initialize_mcp_server(&server_name, &server_config, silent_mode).await;

        // Store the result
        results.push(result);
    }

    Ok(results)
}

/// Initialize a single MCP server from configuration
async fn initialize_mcp_server(
    server_name: &str,
    config: &McpServerConfig,
    silent_mode: bool,
) -> ToolResult {
    use crate::mcp::tool_provider::McpToolProvider;
    use crate::tools::mcp::MCP_PROVIDERS;
    use std::sync::Arc;

    // Extract command and arguments
    let executable = &config.command;
    let args_slice: Vec<&str> = config.args.iter().map(|s| s.as_str()).collect();
    
    // Log environment variables if present
    if !config.env.is_empty() && !silent_mode {
        bprintln!(
            "🌐 Setting {} environment variables for MCP server '{}'",
            config.env.len(),
            server_name
        );
    }

    // Check if already connected
    {
        let providers = MCP_PROVIDERS.lock().await;
        if providers.contains_key(server_name) {
            if !silent_mode {
                bprintln !(tool: "mcp",
                    "Already connected to MCP server: {}",
                    server_name
                );
            }
            return ToolResult::success(format!(
                "Already connected to MCP server: {}",
                server_name
            ));
        }
    }

    // Create provider with environment variables
    match McpToolProvider::new_process_with_env(server_name, executable, &args_slice, &config.env).await {
        Ok(provider) => {
            let provider: Arc<McpToolProvider> = Arc::new(provider);

            // Get tool count
            let tool_count = provider.list_tools().await.len();

            // Store provider using the friendly server name
            {
                let mut providers = MCP_PROVIDERS.lock().await;
                providers.insert(server_name.to_string(), provider);
            }

            if !silent_mode {
                bprintln !(tool: "mcp",
                    "Connected to MCP server: {}. Found {} tools.",
                    server_name,
                    tool_count
                );
            }

            ToolResult::success(format!(
                "Connected to MCP server: {}. Found {} tools.",
                server_name, tool_count
            ))
        }
        Err(err) => {
            if !silent_mode {
                bprintln !(error:
                    "Failed to connect to MCP server {}: {}",
                    server_name,
                    err
                );
            }

            ToolResult::error(format!(
                "Failed to connect to MCP server {}: {}",
                server_name, err
            ))
        }
    }
}
