pub mod analytics;
pub mod compile;
pub mod models;
pub mod optimization;
pub mod pipeline;
pub mod storage;
pub mod tools;

pub mod mcp_server;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, serde::Serialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, serde::Serialize)]
pub struct ServerCapabilities {
    pub experimental: serde_json::Value,
    pub tools: ToolCapabilities,
}

#[derive(Debug, serde::Serialize)]
pub struct ToolCapabilities {
    pub list_changed: bool,
}
