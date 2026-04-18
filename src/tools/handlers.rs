use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::compile::MqlCompiler;
use crate::models::Config;
use crate::pipeline::backtest::{BacktestParams, BacktestPipeline};

#[derive(Debug)]
pub struct ToolHandler {
    config: Config,
}

impl ToolHandler {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn handle(&self, name: &str, args: &Value) -> Result<Value> {
        match name {
            "verify_setup" => self.handle_verify_setup().await,
            "list_symbols" => self.handle_list_symbols().await,
            "list_experts" => self.handle_list_experts(args).await,
            "run_backtest" => self.handle_run_backtest(args).await,
            "compile_ea" => self.handle_compile_ea(args).await,
            "get_backtest_status" => self.handle_get_backtest_status(args).await,
            "cache_status" => self.handle_cache_status().await,
            "clean_cache" => self.handle_clean_cache(args).await,
            "list_reports" => self.handle_list_reports(args).await,
            "prune_reports" => self.handle_prune_reports(args).await,
            "list_set_files" => self.handle_list_set_files().await,
            "describe_sweep" => self.handle_describe_sweep(args).await,
            _ => Ok(json!({
                "content": [{ "type": "text", "text": format!("Tool '{}' not implemented", name) }],
                "isError": true
            })),
        }
    }

    async fn handle_verify_setup(&self) -> Result<Value> {
        let mut checks = HashMap::new();
        let mut all_ok = true;

        let config_path = Config::get_config_path();
        checks.insert("config_file", json!({
            "ok": config_path.exists(),
            "detail": config_path.to_string_lossy()
        }));
        if !config_path.exists() {
            all_ok = false;
        }

        if let Some(wine) = &self.config.wine_executable {
            let wine_ok = Path::new(wine).exists();
            checks.insert("wine_executable", json!({ "ok": wine_ok, "detail": wine }));
            if !wine_ok { all_ok = false; }
        } else {
            checks.insert("wine_executable", json!({ "ok": false, "detail": "not set" }));
            all_ok = false;
        }

        if let Some(term) = &self.config.terminal_dir {
            let term_ok = Path::new(term).is_dir();
            checks.insert("terminal_dir", json!({ "ok": term_ok, "detail": term }));
            if !term_ok { all_ok = false; }
        } else {
            checks.insert("terminal_dir", json!({ "ok": false, "detail": "not set" }));
            all_ok = false;
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "all_ok": all_ok,
                "checks": checks,
                "hint": if all_ok { "Environment looks good" } else { "Run: bash scripts/setup.sh" }
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_list_symbols(&self) -> Result<Value> {
        let symbols = vec!["XAUUSD", "EURUSD", "GBPUSD", "USDJPY", "AUDUSD"];
        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "active_server": "Demo",
                "symbols": symbols
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_list_experts(&self, args: &Value) -> Result<Value> {
        let filter = args.get("filter").and_then(|v| v.as_str());
        
        let mut experts = Vec::new();
        
        if let Some(experts_dir) = &self.config.experts_dir {
            if let Ok(entries) = fs::read_dir(experts_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "ex5").unwrap_or(false) {
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        if let Some(f) = filter {
                            if !name.to_lowercase().contains(&f.to_lowercase()) {
                                continue;
                            }
                        }
                        
                        experts.push(json!({
                            "name": name,
                            "path": path.to_string_lossy()
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({ "experts": experts }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_run_backtest(&self, args: &Value) -> Result<Value> {
        let expert = args.get("expert")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

        let symbol = args.get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("XAUUSD");

        let from_date = args.get("from_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("from_date is required"))?;

        let to_date = args.get("to_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("to_date is required"))?;

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

        let pipeline = BacktestPipeline::new(self.config.clone());
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

    async fn handle_compile_ea(&self, args: &Value) -> Result<Value> {
        let expert_path = args.get("expert_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("expert_path is required"))?;

        let compiler = MqlCompiler::new(self.config.clone());
        let result = compiler.compile(expert_path)?;

        if result.success {
            let path_str = result.ex5_path.as_ref().map(|p| p.to_string_lossy().to_string());
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": true,
                    "ex5_path": path_str,
                    "binary_size": result.binary_size,
                    "warnings": result.warnings.len()
                }).to_string() }],
                "isError": false
            }))
        } else {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "errors": result.errors
                }).to_string() }],
                "isError": true
            }))
        }
    }

    async fn handle_get_backtest_status(&self, args: &Value) -> Result<Value> {
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

    async fn handle_cache_status(&self) -> Result<Value> {
        let cache_dir = self.config.tester_cache_dir.as_ref()
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

    async fn handle_clean_cache(&self, args: &Value) -> Result<Value> {
        let _symbol = args.get("symbol").and_then(|v| v.as_str());
        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

        let cache_dir = self.config.tester_cache_dir.as_ref()
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

    async fn handle_list_reports(&self, args: &Value) -> Result<Value> {
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
        
        let reports_dir = self.config.reports_dir();
        let mut reports = Vec::new();

        if let Ok(entries) = fs::read_dir(&reports_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by(|a, b| {
                b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
                    .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH))
            });

            for entry in entries.into_iter().take(limit) {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    let metrics_file = path.join("metrics.json");
                    let mut profit = 0.0;
                    let mut dd = 0.0;
                    let mut trades = 0;

                    if let Ok(content) = fs::read_to_string(&metrics_file) {
                        if let Ok(metrics) = serde_json::from_str::<Value>(&content) {
                            profit = metrics.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            dd = metrics.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            trades = metrics.get("total_trades").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        }
                    }

                    reports.push(json!({
                        "name": name,
                        "profit": profit,
                        "max_dd_pct": dd,
                        "trades": trades
                    }));
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({ "reports": reports }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_prune_reports(&self, args: &Value) -> Result<Value> {
        let keep_last = args.get("keep_last").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        
        let reports_dir = self.config.reports_dir();
        let mut pruned = 0;

        if let Ok(entries) = fs::read_dir(&reports_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by(|a, b| {
                b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
                    .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH))
            });

            for entry in entries.into_iter().skip(keep_last) {
                let path = entry.path();
                if path.is_dir() && !path.to_string_lossy().ends_with("_opt") {
                    let _ = fs::remove_dir_all(&path);
                    pruned += 1;
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "pruned": pruned,
                "kept": keep_last
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_list_set_files(&self) -> Result<Value> {
        let mut set_files = Vec::new();

        if let Some(tester_dir) = &self.config.tester_profiles_dir {
            if let Ok(entries) = fs::read_dir(tester_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "set").unwrap_or(false) {
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let content = fs::read_to_string(&path).unwrap_or_default();
                        let param_count = content.lines().filter(|l| l.contains(':')).count();
                        let sweep_count = content.lines().filter(|l| l.contains("||Y")).count();

                        set_files.push(json!({
                            "name": name,
                            "path": path.to_string_lossy(),
                            "param_count": param_count,
                            "sweep_count": sweep_count
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({ "set_files": set_files }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_describe_sweep(&self, args: &Value) -> Result<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("path is required"))?;

        let content = fs::read_to_string(path)?;
        let mut sweep_params = serde_json::Map::new();

        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                
                if value.contains("||Y") {
                    if let Some((from_val, to_val)) = value.split_once("..") {
                        sweep_params.insert(key.to_string(), json!({
                            "from": from_val.trim(),
                            "to": to_val.trim().replace("||Y", ""),
                            "step": 1.0
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "path": path,
                "sweep_params": sweep_params
            }).to_string() }],
            "isError": false
        }))
    }
}
