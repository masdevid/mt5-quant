use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::{models::Config as ModelsConfig, tools::ToolHandler, McpError, McpRequest, McpResponse};

#[allow(dead_code)]
type NotificationCallback = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;

/// Auto-verify result stored after first initialization
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AutoVerifyResult {
    all_ok: bool,
    hint: String,
    config_path: String,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub method: String,
    pub params: Value,
}

pub struct McpServer {
    initialized: Arc<Mutex<bool>>,
    tool_handler: Arc<Mutex<Option<ToolHandler>>>,
    auto_verify_result: Arc<Mutex<Option<AutoVerifyResult>>>,
    notification_tx: Arc<Mutex<Option<mpsc::UnboundedSender<Notification>>>>,
}

impl McpServer {
    pub fn new() -> Self {
        let config = ModelsConfig::load().unwrap_or_default();
        Self {
            initialized: Arc::new(Mutex::new(false)),
            tool_handler: Arc::new(Mutex::new(Some(ToolHandler::new(config)))),
            auto_verify_result: Arc::new(Mutex::new(None)),
            notification_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_notification_sender(&self, tx: mpsc::UnboundedSender<Notification>) {
        let mut guard = self.notification_tx.lock().await;
        *guard = Some(tx.clone());
        
        // Update tool handler with notification callback
        let tx_clone = tx.clone();
        let callback = Arc::new(move |method: &str, params: serde_json::Value| {
            let _ = tx_clone.send(Notification {
                method: method.to_string(),
                params,
            });
        });
        
        let config = ModelsConfig::load().unwrap_or_default();
        let new_handler = ToolHandler::with_notification_callback(config, callback);
        
        let mut handler_guard = self.tool_handler.lock().await;
        *handler_guard = Some(new_handler);
    }

    #[allow(dead_code)]
    pub async fn get_notification_sender(&self) -> Option<mpsc::UnboundedSender<Notification>> {
        let guard = self.notification_tx.lock().await;
        guard.clone()
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
                    result: Some(json!({"tools": crate::tools::get_tools_list()})),
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
        let handler_guard = self.tool_handler.lock().await;
        let handler = handler_guard.as_ref().cloned();
        drop(handler_guard);
        
        match handler {
            Some(h) => {
                h.handle(tool_name, arguments).await.unwrap_or_else(|e| json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Tool execution failed: {}", e)
                    }],
                    "isError": true
                }))
            }
            None => json!({
                "content": [{
                    "type": "text",
                    "text": "Tool handler not initialized"
                }],
                "isError": true
            })
        }
    }
}
