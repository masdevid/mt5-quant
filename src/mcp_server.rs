use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{models::Config as ModelsConfig, tools::ToolHandler, McpError, McpRequest, McpResponse};

/// Auto-verify result stored after first initialization
#[derive(Debug, Clone)]
struct AutoVerifyResult {
    all_ok: bool,
    hint: String,
    config_path: String,
}

#[derive(Debug)]
pub struct McpServer {
    initialized: Arc<Mutex<bool>>,
    tool_handler: Arc<ToolHandler>,
    auto_verify_result: Arc<Mutex<Option<AutoVerifyResult>>>,
}

impl McpServer {
    pub fn new() -> Self {
        let config = ModelsConfig::load().unwrap_or_default();
        Self {
            initialized: Arc::new(Mutex::new(false)),
            tool_handler: Arc::new(ToolHandler::new(config)),
            auto_verify_result: Arc::new(Mutex::new(None)),
        }
    }

    /// Run verify_setup on initialization and return summary
    async fn run_auto_verify(&self) -> AutoVerifyResult {
        // Get config from tool_handler
        let config = ModelsConfig::load().unwrap_or_default();
        
        // Check if config exists
        let config_path = ModelsConfig::writable_config_path();
        let config_exists = config_path.exists();
        
        // Check wine and terminal
        let wine_ok = config.wine_executable.as_ref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);
        let term_ok = config.terminal_dir.as_ref()
            .map(|p| std::path::Path::new(p).is_dir())
            .unwrap_or(false);
        
        let all_ok = config_exists && wine_ok && term_ok;
        
        let hint = if all_ok {
            "Environment fully configured and ready".to_string()
        } else if !config_exists {
            format!("Auto-discovery will run on first request. Config will be written to {}", config_path.display())
        } else if !wine_ok {
            "Wine/CrossOver not found - required for MT5 execution".to_string()
        } else if !term_ok {
            "MT5 directory not found - check installation".to_string()
        } else {
            "Fix missing paths in config".to_string()
        };
        
        AutoVerifyResult {
            all_ok,
            hint,
            config_path: config_path.to_string_lossy().to_string(),
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
                
                // Run auto-verify on first initialization
                let verify_result = self.run_auto_verify().await;
                let all_ok = verify_result.all_ok;
                let hint = verify_result.hint.clone();
                
                // Store the result
                *self.auto_verify_result.lock().await = Some(verify_result);
                
                *self.initialized.lock().await = true;
                
                // Include verify status in server info
                let server_info = json!({
                    "name": "MT5-Quant",
                    "version": "1.27.0",
                    "setup": {
                        "verified": all_ok,
                        "hint": hint,
                    }
                });
                
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "protocolVersion": negotiated_version,
                        "capabilities": {
                            "experimental": {},
                            "tools": {
                                "listChanged": false,
                            },
                        },
                        "serverInfo": server_info,
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
