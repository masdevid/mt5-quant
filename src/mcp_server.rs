use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{config::Config, mt5::Mt5Manager, McpError, McpRequest, McpResponse};

#[derive(Debug)]
pub struct McpServer {
    initialized: Arc<Mutex<bool>>,
    mt5_manager: Arc<Mt5Manager>,
}

impl McpServer {
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        Self {
            initialized: Arc::new(Mutex::new(false)),
            mt5_manager: Arc::new(Mt5Manager::new(config)),
        }
    }

    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => {
                *self.initialized.lock().await = true;
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!(crate::InitializeResult {
                        protocol_version: "2024-11-05".to_string(),
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
                    result: Some(crate::get_tools_list()),
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
        match tool_name {
            "verify_setup" => {
                self.mt5_manager.verify_setup().await.unwrap_or_else(|e| json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Setup verification failed: {}", e)
                    }],
                    "isError": true
                }))
            }
            "list_symbols" => {
                self.mt5_manager.list_symbols().await.unwrap_or_else(|e| json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Failed to list symbols: {}", e)
                    }],
                    "isError": true
                }))
            }
            "list_experts" => {
                let filter = arguments.get("filter").and_then(|v| v.as_str());
                self.mt5_manager.list_experts(filter).await.unwrap_or_else(|e| json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Failed to list experts: {}", e)
                    }],
                    "isError": true
                }))
            }
            "run_backtest" => {
                self.mt5_manager.run_backtest(arguments).await.unwrap_or_else(|e| json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Backtest failed: {}", e)
                    }],
                    "isError": true
                }))
            }
            "compile_ea" => {
                let expert_path = arguments.get("expert_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                self.mt5_manager.compile_ea(expert_path).await.unwrap_or_else(|e| json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Compilation failed: {}", e)
                    }],
                    "isError": true
                }))
            }
            _ => {
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Tool '{}' not found", tool_name)
                    }],
                    "isError": true
                })
            }
        }
    }
}
