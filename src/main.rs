//! MCP GDB Server
//!
//! A Model Context Protocol (MCP) server that provides GDB debugging capabilities
//! for LLMs. Supports gdb-multiarch and remote debugging targets.
//!
//! Usage:
//!   Add to Claude Desktop config:
//!   ```json
//!   {
//!     "mcpServers": {
//!       "gdb": {
//!         "command": "/path/to/mcp-gdb-server"
//!       }
//!     }
//!   }
//!   ```

mod gdb;
mod mcp;

use crate::mcp::protocol::*;
use crate::mcp::GdbMcpServer;
use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

/// MCP Server state
struct ServerState {
    server: GdbMcpServer,
    initialized: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting MCP GDB Server v0.1.0");

    let state = RwLock::new(ServerState {
        server: GdbMcpServer::new(),
        initialized: false,
    });

    // Read from stdin, write to stdout
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    let reader = BufReader::new(stdin);

    info!("MCP GDB Server ready, listening on stdin");

    for line in reader.lines() {
        match line {
            Ok(line) => {
                debug!("Received: {}", line);

                // Parse the JSON-RPC request
                let request: Result<JsonRpcRequest, _> = serde_json::from_str(&line);

                match request {
                    Ok(req) => {
                        let response = handle_request(&state, req).await;

                        match response {
                            Ok(Some(resp)) => {
                                let resp_str = serde_json::to_string(&resp)?;
                                debug!("Sending: {}", resp_str);
                                writeln!(stdout, "{}", resp_str)?;
                                stdout.flush()?;
                            }
                            Ok(None) => {
                                // Notification, no response needed
                            }
                            Err(e) => {
                                error!("Error handling request: {}", e);
                                let error_resp = JsonRpcErrorResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: None,
                                    error: JsonRpcError::internal_error(&e.to_string()),
                                };
                                let resp_str = serde_json::to_string(&error_resp)?;
                                writeln!(stdout, "{}", resp_str)?;
                                stdout.flush()?;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse request: {}", e);
                        let error_resp = JsonRpcErrorResponse {
                            jsonrpc: "2.0".to_string(),
                            id: None,
                            error: JsonRpcError::parse_error(),
                        };
                        let resp_str = serde_json::to_string(&error_resp)?;
                        writeln!(stdout, "{}", resp_str)?;
                        stdout.flush()?;
                    }
                }
            }
            Err(e) => {
                error!("Error reading from stdin: {}", e);
                break;
            }
        }
    }

    info!("MCP GDB Server shutting down");
    Ok(())
}

/// Handle a JSON-RPC request
async fn handle_request(
    state: &RwLock<ServerState>,
    request: JsonRpcRequest,
) -> Result<Option<JsonRpcResponse>> {
    let method = request.method.as_str();

    debug!("Handling method: {}", method);

    match method {
        // MCP Protocol methods
        "initialize" => {
            let mut state = state.write().await;
            state.initialized = true;
            let result = state.server.handle_initialize(request.params).await?;
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result,
            }))
        }
        "initialized" => {
            // Notification, no response needed
            Ok(None)
        }
        "ping" => {
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result: serde_json::json!({}),
            }))
        }
        "tools/list" => {
            let state = state.read().await;
            let result = state.server.handle_tools_list().await?;
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result,
            }))
        }
        "tools/call" => {
            let state = state.read().await;
            let result = state.server.handle_tools_call(request.params).await?;
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result,
            }))
        }
        "resources/list" => {
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result: serde_json::json!({"resources": []}),
            }))
        }
        "prompts/list" => {
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result: serde_json::json!({"prompts": []}),
            }))
        }
        "logging/setLevel" => {
            // Acknowledge but ignore
            Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(RequestId::String("0".to_string())),
                result: serde_json::json!({}),
            }))
        }
        _ => {
            warn!("Unknown method: {}", method);
            Err(anyhow::anyhow!("Unknown method: {}", method))
        }
    }
}
