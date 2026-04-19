use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{models::Config as ModelsConfig, tools::ToolHandler, McpError, McpRequest, McpResponse};

/// Auto-verify result stored after first initialization
#[derive(Debug, Clone)]
#[allow(dead_code)]
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

    /// Run verify_setup in background - non blocking
    fn spawn_auto_verify(&self) {
        let result_arc = self.auto_verify_result.clone();
        
        tokio::spawn(async move {
            // Get config
            let config = ModelsConfig::load().unwrap_or_default();
            let config_path = ModelsConfig::writable_config_path();
            
            // Quick async file checks
            let config_exists = tokio::task::spawn_blocking({
                let path = config_path.clone();
                move || path.exists()
            }).await.unwrap_or(false);
            
            let wine_ok = if let Some(wine) = &config.wine_executable {
                let wine = wine.clone();
                tokio::task::spawn_blocking(move || {
                    std::path::Path::new(&wine).exists()
                }).await.unwrap_or(false)
            } else {
                false
            };
            
            let term_ok = if let Some(term) = &config.terminal_dir {
                let term = term.clone();
                tokio::task::spawn_blocking(move || {
                    std::path::Path::new(&term).is_dir()
                }).await.unwrap_or(false)
            } else {
                false
            };
            
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
            
            let result = AutoVerifyResult {
                all_ok,
                hint,
                config_path: config_path.to_string_lossy().to_string(),
            };
            
            // Store result
            let mut guard = result_arc.lock().await;
            *guard = Some(result);
        });
    }
    
    /// Get current verify status (may be loading if called immediately after init)
    #[allow(dead_code)]
    async fn get_verify_status(&self) -> (Option<bool>, String) {
        let guard = self.auto_verify_result.lock().await;
        match guard.as_ref() {
            Some(result) => (Some(result.all_ok), result.hint.clone()),
            None => (None, "Checking environment...".to_string()),
        }
    }

    /// Handle a notification (no id — no response sent)
    pub async fn handle_notification(&self, request: McpRequest) {
        match request.method.as_str() {
            "notifications/initialized" => {
                // Client confirms initialization is complete — no action needed
            }
            _ => {
                tracing::debug!("Unhandled notification: {}", request.method);
            }
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
                
                // Start background verify (non-blocking)
                self.spawn_auto_verify();
                
                *self.initialized.lock().await = true;
                
                // Return immediately with fast status
                let server_info = json!({
                    "name": "MT5-Quant",
                    "version": env!("CARGO_PKG_VERSION"),
                    "setup": {
                        "hint": "Auto-verification running... Use verify_setup tool for detailed status",
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
