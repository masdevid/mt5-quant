mod analytics;
mod compile;
mod models;
mod optimization;
mod pipeline;
mod storage;
mod tools;

mod config;
mod mcp_server;

use anyhow::Result;
use clap::Parser;
use serde_json::{json, Value};
use std::io::{stdout, Write};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{info, error};

#[derive(Parser)]
#[command(name = "mt5-quant")]
#[command(about = "MT5-Quant MCP Server - Exposes MT5 backtest and optimization tools via MCP")]
struct Cli {
    /// Run as stdio MCP server (default)
    #[arg(short, long, default_value = "false")]
    stdio: bool,
    
    /// Run on TCP port for debugging
    #[arg(short, long)]
    port: Option<u16>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpResponse {
    jsonrpc: String,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<McpError>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, serde::Serialize)]
pub struct InitializeResult {
    protocol_version: String,
    capabilities: ServerCapabilities,
    server_info: ServerInfo,
}

#[derive(Debug, serde::Serialize)]
pub struct ServerCapabilities {
    experimental: Value,
    tools: ToolCapabilities,
}

#[derive(Debug, serde::Serialize)]
pub struct ToolCapabilities {
    list_changed: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    if let Some(port) = cli.port {
        run_tcp_server(port).await?;
    } else {
        run_stdio_server().await?;
    }
    
    Ok(())
}

async fn run_stdio_server() -> Result<()> {
    info!("Starting MT5-Quant MCP server on stdio");
    
    let server = std::sync::Arc::new(mcp_server::McpServer::new());
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut line = String::new();
    
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                
                let server_clone = server.clone();
                match serde_json::from_str::<McpRequest>(line) {
                    Ok(request) => {
                        let response = server_clone.handle_request(request).await;
                        let response_json = serde_json::to_string(&response)?;
                        println!("{}", response_json);
                        stdout().flush()?;
                    }
                    Err(e) => {
                        error!("Failed to parse JSON: {}", e);
                        let error_response = McpResponse {
                            jsonrpc: "2.0".to_string(),
                            id: None,
                            result: None,
                            error: Some(McpError {
                                code: -32700,
                                message: "Parse error".to_string(),
                                data: Some(json!(e.to_string())),
                            }),
                        };
                        let response_json = serde_json::to_string(&error_response)?;
                        println!("{}", response_json);
                        stdout().flush()?;
                    }
                }
            }
            Err(e) => {
                error!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

async fn run_tcp_server(port: u16) -> Result<()> {
    info!("Starting MT5-Quant MCP server on TCP port {}", port);
    
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    info!("Listening on 127.0.0.1:{}", port);
    
    loop {
        let (socket, addr) = listener.accept().await?;
        info!("New connection from {}", addr);
        
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                error!("Connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(socket: tokio::net::TcpStream) -> Result<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let server = mcp_server::McpServer::new();
    
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                
                match serde_json::from_str::<McpRequest>(line) {
                    Ok(request) => {
                        let response = server.handle_request(request).await;
                        let response_json = serde_json::to_string(&response)? + "\n";
                        writer.write_all(response_json.as_bytes()).await?;
                    }
                    Err(e) => {
                        error!("Failed to parse JSON: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to read: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}
