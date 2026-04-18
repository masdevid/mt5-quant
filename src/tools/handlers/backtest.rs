use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use crate::models::Config;
use crate::pipeline::backtest::{BacktestParams, BacktestPipeline};

pub async fn handle_run_backtest(config: &Config, args: &Value) -> Result<Value> {
    let expert = args.get("expert")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

    // Symbol pre-flight
    let requested_symbol = args.get("symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let available = config.discover_symbols();

    let symbol = if requested_symbol.is_empty() {
        let default = config.backtest_symbol.clone()
            .unwrap_or_else(|| "XAUUSD".to_string());
        if available.contains(&default) {
            default
        } else if let Some(first) = available.first() {
            tracing::warn!("Default symbol {} not found; using {}", default, first);
            first.clone()
        } else {
            default
        }
    } else {
        if !available.is_empty() && !available.contains(&requested_symbol.to_string()) {
            return Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "error": format!("Symbol '{}' has no local history data.", requested_symbol),
                    "available_symbols": available,
                    "hint": "Use list_symbols to see all available symbols."
                }).to_string() }],
                "isError": true
            }));
        }
        requested_symbol.to_string()
    };

    // Date defaulting: past complete calendar month
    let (from_date, to_date) = {
        let f = args.get("from_date").and_then(|v| v.as_str()).unwrap_or("");
        let t = args.get("to_date").and_then(|v| v.as_str()).unwrap_or("");
        if f.is_empty() || t.is_empty() {
            super::past_complete_month()
        } else {
            (f.to_string(), t.to_string())
        }
    };

    let params = BacktestParams {
        expert: expert.to_string(),
        symbol: symbol.to_string(),
        from_date: from_date.to_string(),
        to_date: to_date.to_string(),
        timeframe: args.get("timeframe").and_then(|v| v.as_str()).unwrap_or("M5").to_string(),
        deposit: args.get("deposit").and_then(|v| v.as_u64()).unwrap_or(10000) as u32,
        model: args.get("model").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
        leverage: args.get("leverage").and_then(|v| v.as_u64()).unwrap_or(500) as u32,
        set_file: args.get("set_file").and_then(|v| v.as_str()).map(|s| s.to_string()),
        skip_compile: args.get("skip_compile").and_then(|v| v.as_bool()).unwrap_or(false),
        skip_clean: args.get("skip_clean").and_then(|v| v.as_bool()).unwrap_or(false),
        skip_analyze: args.get("skip_analyze").and_then(|v| v.as_bool()).unwrap_or(false),
        deep_analyze: args.get("deep").and_then(|v| v.as_bool()).unwrap_or(false),
        shutdown: args.get("shutdown").and_then(|v| v.as_bool()).unwrap_or(false),
        kill_existing: args.get("kill_existing").and_then(|v| v.as_bool()).unwrap_or(false),
        timeout: args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(900),
        gui: args.get("gui").and_then(|v| v.as_bool()).unwrap_or(false),
    };

    let pipeline = BacktestPipeline::new(config.clone());
    let result = pipeline.run(params).await?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": result.success,
            "report_dir": result.report_dir.to_string_lossy(),
            "duration_seconds": result.duration_seconds,
            "message": result.message
        }).to_string() }],
        "isError": !result.success
    }))
}

pub async fn handle_get_backtest_status(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("latest");

    let progress_file = Path::new(report_dir).join("progress.log");
    
    let status = if progress_file.exists() {
        if let Ok(content) = fs::read_to_string(&progress_file) {
            let last_line = content.lines().last().unwrap_or("");
            if last_line.contains("DONE") {
                "completed"
            } else {
                "running"
            }
        } else {
            "unknown"
        }
    } else {
        "not_started"
    };

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "report_dir": report_dir,
            "status": status
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_cache_status(config: &Config) -> Result<Value> {
    let cache_dir = config.tester_cache_dir.as_ref()
        .map(|s| Path::new(s))
        .filter(|p| p.exists());

    let mut total_size: u64 = 0;
    let mut symbols = Vec::new();

    if let Some(dir) = cache_dir {
        for entry in walkdir::WalkDir::new(dir).max_depth(2) {
            if let Ok(entry) = entry {
                if entry.file_type().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        symbols.push(name.to_string());
                    }
                } else {
                    if let Ok(meta) = entry.metadata() {
                        total_size += meta.len();
                    }
                }
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "cache_dir": cache_dir.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            "total_bytes": total_size,
            "symbols": symbols
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_clean_cache(config: &Config, args: &Value) -> Result<Value> {
    let _symbol = args.get("symbol").and_then(|v| v.as_str());
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

    let cache_dir = config.tester_cache_dir.as_ref()
        .map(|s| Path::new(s))
        .filter(|p| p.exists());

    let mut bytes_freed: u64 = 0;

    if let Some(dir) = cache_dir {
        for entry in walkdir::WalkDir::new(dir) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().map(|e| e == "tst").unwrap_or(false) {
                    if let Ok(meta) = entry.metadata() {
                        bytes_freed += meta.len();
                        if !dry_run {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "bytes_freed": bytes_freed,
            "dry_run": dry_run
        }).to_string() }],
        "isError": false
    }))
}
