mod analytics;
mod compile;
mod models;
mod optimization;
mod pipeline;
mod storage;
mod tools;

mod mcp_server;

use anyhow::Result;
use clap::Parser;
use serde_json::{json, Value};
use std::io::{stdout, Write};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{info, error};

use crate::models::Config;
use crate::pipeline::backtest::{BacktestPipeline, BacktestParams};

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
    
    /// Test backtest launch performance (direct Rust call, not MCP)
    #[arg(long)]
    test_launch: bool,
    
    /// EA name for test launch
    #[arg(long)]
    ea: Option<String>,
    
    /// Startup delay for test launch (default: 10)
    #[arg(long)]
    startup_delay: Option<u64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct McpError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    
    let cli = Cli::parse();
    
    if cli.test_launch {
        run_test_launch(cli.ea, cli.startup_delay).await?;
        return Ok(());
    }
    
    if let Some(port) = cli.port {
        run_tcp_server(port).await?;
    } else {
        run_stdio_server().await?;
    }
    
    Ok(())
}

async fn run_test_launch(ea: Option<String>, startup_delay: Option<u64>) -> Result<()> {
    let expert = ea.ok_or_else(|| anyhow::anyhow!("--ea is required for test launch"))?;
    let delay = startup_delay.unwrap_or(10);
    
    println!("Testing MT5 backtest launch optimizations...");
    println!("==============================================");
    println!("EA: {}", expert);
    println!("Startup delay: {}s", delay);
    
    let config = Config::load()?;
    
    let params = BacktestParams {
        expert: expert.clone(),
        symbol: "XAUUSD".to_string(),
        from_date: "2024.01.01".to_string(),
        to_date: "2024.01.31".to_string(),
        timeframe: "M5".to_string(),
        deposit: 10000,
        model: 0,
        leverage: 500,
        set_file: None,
        skip_compile: true,
        skip_clean: false,
        skip_analyze: true,
        deep_analyze: false,
        shutdown: false,
        kill_existing: false,
        timeout: 900,
        gui: false,
        startup_delay_secs: delay,
    };
    
    let pipeline = BacktestPipeline::new(config);
    
    println!("\nLaunching backtest...");
    let start = Instant::now();
    match pipeline.launch_backtest(params).await {
        Ok(job) => {
            let elapsed = start.elapsed();
            println!("✓ Launch completed in {:.2}s", elapsed.as_secs_f64());
            println!("  Report ID: {}", job.report_id);
            println!("  Report dir: {}", job.report_dir);
            println!("\nUse get_backtest_status to monitor progress.");
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("✗ Launch failed after {:.2}s: {}", elapsed.as_secs_f64(), e);
        }
    }
    
    println!("\n==============================================");
    println!("Test complete.");
    
    Ok(())
}

async fn run_stdio_server() -> Result<()> {
    info!("Starting MT5-Quant MCP server on stdio");
    
    let server = std::sync::Arc::new(mcp_server::McpServer::new());
    let (notification_tx, mut notification_rx) = tokio::sync::mpsc::unbounded_channel::<mcp_server::Notification>();
    server.set_notification_sender(notification_tx).await;
    
    // Spawn notification sender task
    tokio::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            let notification_json = json!({
                "jsonrpc": "2.0",
                "method": notification.method,
                "params": notification.params,
            });
            println!("{}", notification_json);
            let _ = stdout().flush();
        }
    });
    
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
                        // Notifications have no id — don't send a response
                        if request.id.is_none() {
                            server_clone.handle_notification(request).await;
                            continue;
                        }
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
                        if request.id.is_none() {
                            server.handle_notification(request).await;
                            continue;
                        }
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
