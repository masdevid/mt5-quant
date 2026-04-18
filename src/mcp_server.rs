use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{models::Config as ModelsConfig, tools::ToolHandler, McpError, McpRequest, McpResponse};

#[derive(Debug)]
pub struct McpServer {
    initialized: Arc<Mutex<bool>>,
    tool_handler: Arc<ToolHandler>,
}

impl McpServer {
    pub fn new() -> Self {
        let config = ModelsConfig::load().unwrap_or_default();
        Self {
            initialized: Arc::new(Mutex::new(false)),
            tool_handler: Arc::new(ToolHandler::new(config)),
        }
    }

    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => {
                // Protocol version negotiation: client sends desired version
                let client_version = request.params.as_ref()
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");
                
                // Server selects version (latest supported that client also supports)
                let negotiated_version = if client_version.starts_with("2025-") {
                    "2024-11-05" // Fall back to stable version
                } else {
                    "2024-11-05"
                };
                
                *self.initialized.lock().await = true;
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!(crate::InitializeResult {
                        protocol_version: negotiated_version.to_string(),
                        capabilities: crate::ServerCapabilities {
                            experimental: json!({}),
                            tools: crate::ToolCapabilities {
                                list_changed: false,
                            },
                        },
                        server_info: crate::ServerInfo {
                            name: "MT5-Quant".to_string(),
                            version: "1.27.0".to_string(),
                        },
                    })),
                    error: None,
                }
            }
            "tools/list" => {
                let initialized = *self.initialized.lock().await;
                if !initialized {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(McpError {
                            code: -32600,
                            message: "Received request before initialization was complete".to_string(),
                            data: None,
                        }),
                    };
                }
                
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(crate::tools::get_tools_list()),
                    error: None,
                }
            }
            "tools/call" => {
                let initialized = *self.initialized.lock().await;
                if !initialized {
                    return McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(McpError {
                            code: -32600,
                            message: "Received request before initialization was complete".to_string(),
                            data: None,
                        }),
                    };
                }
                
                if let Some(params) = request.params {
                    if let (Some(tool_name), Some(arguments)) = (
                        params.get("name").and_then(|v| v.as_str()),
                        params.get("arguments")
                    ) {
                        let result = self.handle_tool_call(tool_name, arguments).await;
                        McpResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(result),
                            error: None,
                        }
                    } else {
                        McpResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(McpError {
                                code: -32602,
                                message: "Invalid request parameters".to_string(),
                                data: None,
                            }),
                        }
                    }
                } else {
                    McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(McpError {
                            code: -32602,
                            message: "Invalid request parameters".to_string(),
                            data: None,
                        }),
                    }
                }
            }
            _ => {
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(McpError {
                        code: -32601,
                        message: format!("Method not found: {}", request.method),
                        data: None,
                    }),
                }
            }
        }
    }

    async fn handle_tool_call(&self, tool_name: &str, arguments: &Value) -> Value {
        self.tool_handler.handle(tool_name, arguments).await.unwrap_or_else(|e| json!({
            "content": [{
                "type": "text",
                "text": format!("Tool execution failed: {}", e)
            }],
            "isError": true
        }))
    }
}
