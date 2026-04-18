use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::analytics::DealAnalyzer;
use crate::compile::MqlCompiler;
use crate::models::Config;
use crate::models::deals::Deal;
use crate::models::metrics::Metrics;
use crate::optimization::{OptimizationParams, OptimizationParser, OptimizationRunner};
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
            // Optimization tools
            "run_optimization" => self.handle_run_optimization(args).await,
            "get_optimization_status" => self.handle_get_optimization_status(args).await,
            "get_optimization_results" => self.handle_get_optimization_results(args).await,
            "list_jobs" => self.handle_list_jobs().await,
            // Analysis tools
            "analyze_report" => self.handle_analyze_report(args).await,
            "compare_baseline" => self.handle_compare_baseline(args).await,
            // Set file tools
            "read_set_file" => self.handle_read_set_file(args).await,
            "write_set_file" => self.handle_write_set_file(args).await,
            "patch_set_file" => self.handle_patch_set_file(args).await,
            "clone_set_file" => self.handle_clone_set_file(args).await,
            "diff_set_files" => self.handle_diff_set_files(args).await,
            "set_from_optimization" => self.handle_set_from_optimization(args).await,
            // Utility tools
            "tail_log" => self.handle_tail_log(args).await,
            "archive_report" => self.handle_archive_report(args).await,
            "archive_all_reports" => self.handle_archive_all_reports(args).await,
            "promote_to_baseline" => self.handle_promote_to_baseline(args).await,
            "get_history" => self.handle_get_history(args).await,
            "annotate_history" => self.handle_annotate_history(args).await,
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

    // Optimization handlers
    async fn handle_run_optimization(&self, args: &Value) -> Result<Value> {
        let expert = args.get("expert")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

        let set_file = args.get("set_file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("set_file is required"))?;

        let from_date = args.get("from_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("from_date is required"))?;

        let to_date = args.get("to_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("to_date is required"))?;

        let params = OptimizationParams {
            expert: expert.to_string(),
            set_file: set_file.to_string(),
            symbol: args.get("symbol").and_then(|v| v.as_str()).unwrap_or("XAUUSD").to_string(),
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
            deposit: args.get("deposit").and_then(|v| v.as_u64()).unwrap_or(10000) as u32,
            model: 0, // Always 0 for optimization
            leverage: args.get("leverage").and_then(|v| v.as_u64()).unwrap_or(500) as u32,
            currency: args.get("currency").and_then(|v| v.as_str()).unwrap_or("USD").to_string(),
        };

        let runner = OptimizationRunner::new(self.config.clone());
        let result = runner.run(params).await?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": result.success,
                "job_id": result.job_id,
                "pid": result.pid,
                "log_file": result.log_file.to_string_lossy(),
                "combinations": result.combinations,
                "message": result.message,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_get_optimization_status(&self, args: &Value) -> Result<Value> {
        let job_id = args.get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("job_id is required"))?;

        let runner = OptimizationRunner::new(self.config.clone());
        let status = runner.get_job_status(job_id)?;

        Ok(json!({
            "content": [{ "type": "text", "text": status.to_string() }],
            "isError": false
        }))
    }

    async fn handle_get_optimization_results(&self, args: &Value) -> Result<Value> {
        let job_id = args.get("job_id")
            .and_then(|v| v.as_str());

        let file = args.get("file")
            .and_then(|v| v.as_str());

        let parser = OptimizationParser::new();
        
        let passes = if let Some(jid) = job_id {
            parser.parse_job(jid)?
        } else if let Some(f) = file {
            parser.parse_file(std::path::Path::new(f))?
        } else {
            return Err(anyhow::anyhow!("Either job_id or file is required"));
        };

        let sort_by = args.get("sort").and_then(|v| v.as_str()).unwrap_or("profit");
        let top_n = args.get("top").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

        // Find best pass
        let best = parser.find_best_pass(&passes, sort_by);

        let mut sorted_passes = passes.clone();
        sorted_passes.sort_by(|a, b| b.profit.partial_cmp(&a.profit).unwrap());
        sorted_passes.truncate(top_n);

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "total_passes": passes.len(),
                "top_passes": sorted_passes,
                "best": best,
                "sort_by": sort_by,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_list_jobs(&self) -> Result<Value> {
        let runner = OptimizationRunner::new(self.config.clone());
        let jobs = runner.list_jobs()?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({ "jobs": jobs }).to_string() }],
            "isError": false
        }))
    }

    // Analysis handlers
    async fn handle_analyze_report(&self, args: &Value) -> Result<Value> {
        let report_dir = args.get("report_dir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

        let deals_csv = std::path::Path::new(report_dir).join("deals.csv");
        let metrics_json = std::path::Path::new(report_dir).join("metrics.json");

        if !deals_csv.exists() {
            return Err(anyhow::anyhow!("deals.csv not found in {}", report_dir));
        }

        // Read deals
        let deals = self.read_deals_from_csv(&deals_csv)?;
        
        // Read metrics
        let metrics = if metrics_json.exists() {
            let content = fs::read_to_string(&metrics_json)?;
            serde_json::from_str(&content)?
        } else {
            Metrics::default()
        };

        let _strategy = args.get("strategy").and_then(|v| v.as_str()).unwrap_or("grid");
        let _deep = args.get("deep").and_then(|v| v.as_bool()).unwrap_or(false);

        let analyzer = DealAnalyzer::new();
        let result = analyzer.analyze(&deals, &metrics);

        // Write analysis.json
        let analysis_path = std::path::Path::new(report_dir).join("analysis.json");
        fs::write(&analysis_path, serde_json::to_string_pretty(&result)?)?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "analysis_file": analysis_path.to_string_lossy(),
                "summary": result,
            }).to_string() }],
            "isError": false
        }))
    }

    fn read_deals_from_csv(&self, path: &std::path::Path) -> Result<Vec<Deal>> {
        let content = fs::read_to_string(path)?;
        let mut deals = Vec::new();
        
        let mut lines = content.lines();
        let _header = lines.next(); // Skip header
        
        for line in lines {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 12 {
                deals.push(Deal {
                    time: parts[0].to_string(),
                    deal: parts[1].to_string(),
                    symbol: parts[2].to_string(),
                    deal_type: parts[3].to_string(),
                    entry: parts[4].to_string(),
                    volume: parts[5].parse().unwrap_or(0.0),
                    price: parts[6].parse().unwrap_or(0.0),
                    order: parts[7].to_string(),
                    commission: parts[8].parse().unwrap_or(0.0),
                    swap: parts[9].parse().unwrap_or(0.0),
                    profit: parts[10].parse().unwrap_or(0.0),
                    balance: parts[11].parse().unwrap_or(0.0),
                    comment: parts.get(12).unwrap_or(&"").to_string(),
                    magic: parts.get(13).map(|s| s.to_string()),
                });
            }
        }
        
        Ok(deals)
    }

    async fn handle_compare_baseline(&self, args: &Value) -> Result<Value> {
        let report_dir = args.get("report_dir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

        let baseline_path = std::path::Path::new("config/baseline.json");
        let metrics_path = std::path::Path::new(report_dir).join("metrics.json");

        if !baseline_path.exists() {
            return Ok(json!({
                "content": [{ "type": "text", "text": "No baseline.json found in config/" }],
                "isError": false
            }));
        }

        let baseline: Value = serde_json::from_str(&fs::read_to_string(baseline_path)?)?;
        let current: Value = serde_json::from_str(&fs::read_to_string(metrics_path)?)?;

        let comparison = json!({
            "baseline": baseline,
            "current": current,
            "improvements": {
                "profit": current.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0) 
                    - baseline.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "drawdown": current.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0)
                    - baseline.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0),
            }
        });

        Ok(json!({
            "content": [{ "type": "text", "text": comparison.to_string() }],
            "isError": false
        }))
    }

    // Set file handlers
    async fn handle_read_set_file(&self, args: &Value) -> Result<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("path is required"))?;

        let content = fs::read_to_string(path)?;
        let mut params = serde_json::Map::new();

        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                
                if value.contains("||Y") {
                    let parts: Vec<&str> = value.split("||").collect();
                    if parts.len() >= 5 {
                        params.insert(key.to_string(), json!({
                            "value": parts[0],
                            "from": parts[1],
                            "step": parts[2],
                            "to": parts[3],
                            "optimize": true,
                        }));
                    }
                } else {
                    params.insert(key.to_string(), json!({ "value": value, "optimize": false }));
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "path": path,
                "parameters": params,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_write_set_file(&self, args: &Value) -> Result<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("path is required"))?;

        let params = args.get("parameters")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("parameters object is required"))?;

        let mut lines = Vec::new();
        for (key, value) in params {
            if let Some(obj) = value.as_object() {
                if obj.get("optimize").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let from_val = obj.get("from").and_then(|v| v.as_str()).unwrap_or("0");
                    let step = obj.get("step").and_then(|v| v.as_str()).unwrap_or("1");
                    let to_val = obj.get("to").and_then(|v| v.as_str()).unwrap_or("0");
                    lines.push(format!("{}={}||{}||{}||{}||Y", key, obj.get("value").and_then(|v| v.as_str()).unwrap_or("0"), from_val, step, to_val));
                } else {
                    lines.push(format!("{}={}", key, obj.get("value").and_then(|v| v.as_str()).unwrap_or("0")));
                }
            }
        }

        fs::write(path, lines.join("\n"))?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "path": path,
                "parameters_written": lines.len(),
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_patch_set_file(&self, args: &Value) -> Result<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("path is required"))?;

        let patches = args.get("patches")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("patches object is required"))?;

        // Read existing file
        let content = fs::read_to_string(path)?;
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let mut patched_count = 0;

        for (key, value) in patches {
            let new_value = if let Some(s) = value.as_str() {
                s.to_string()
            } else if let Some(n) = value.as_f64() {
                n.to_string()
            } else if let Some(b) = value.as_bool() {
                if b { "true".to_string() } else { "false".to_string() }
            } else {
                value.to_string()
            };

            // Find and patch the parameter
            let mut found = false;
            for line in &mut lines {
                if line.starts_with(&format!("{}:", key)) {
                    *line = format!("{}: {}", key, new_value);
                    found = true;
                    patched_count += 1;
                    break;
                } else if line.starts_with(&format!("{}=", key)) {
                    *line = format!("{}={}", key, new_value);
                    found = true;
                    patched_count += 1;
                    break;
                }
            }

            // If not found, add it
            if !found {
                lines.push(format!("{}: {}", key, new_value));
                patched_count += 1;
            }
        }

        fs::write(path, lines.join("\n"))?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "path": path,
                "parameters_patched": patched_count,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_clone_set_file(&self, args: &Value) -> Result<Value> {
        let source = args.get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("source is required"))?;

        let destination = args.get("destination")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("destination is required"))?;

        fs::copy(source, destination)?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "source": source,
                "destination": destination,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_diff_set_files(&self, args: &Value) -> Result<Value> {
        let file_a = args.get("file_a")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("file_a is required"))?;

        let file_b = args.get("file_b")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("file_b is required"))?;

        let content_a = fs::read_to_string(file_a)?;
        let content_b = fs::read_to_string(file_b)?;

        let mut differences = Vec::new();

        for (i, (line_a, line_b)) in content_a.lines().zip(content_b.lines()).enumerate() {
            if line_a != line_b {
                differences.push(json!({
                    "line": i + 1,
                    "file_a": line_a,
                    "file_b": line_b,
                }));
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "file_a": file_a,
                "file_b": file_b,
                "differences": differences,
                "total_differences": differences.len(),
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_set_from_optimization(&self, args: &Value) -> Result<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("path is required"))?;

        let params = args.get("params")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("params is required"))?;

        let mut lines = Vec::new();
        for (key, value) in params {
            if let Some(val_str) = value.as_str() {
                lines.push(format!("{}={}", key, val_str));
            }
        }

        fs::write(path, lines.join("\n"))?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "path": path,
                "parameters_written": lines.len(),
            }).to_string() }],
            "isError": false
        }))
    }

    // Utility handlers
    async fn handle_tail_log(&self, args: &Value) -> Result<Value> {
        let job_id = args.get("job_id")
            .and_then(|v| v.as_str());

        let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        let log_path = if let Some(jid) = job_id {
            let jobs_dir = std::path::Path::new(".mt5mcp_jobs");
            let meta_path = jobs_dir.join(format!("{}.json", jid));
            let meta: Value = serde_json::from_str(&fs::read_to_string(meta_path)?)?;
            meta.get("log_file").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            args.get("file").and_then(|v| v.as_str()).map(|s| s.to_string())
        };

        let log_path = log_path.ok_or_else(|| anyhow::anyhow!("Could not determine log file"))?;

        let content = fs::read_to_string(&log_path)?;
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines);
        let last_lines = &all_lines[start..];

        Ok(json!({
            "content": [{ "type": "text", "text": last_lines.join("\n") }],
            "isError": false
        }))
    }

    async fn handle_archive_report(&self, args: &Value) -> Result<Value> {
        let report_dir = args.get("report_dir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

        let delete_after = args.get("delete_after").and_then(|v| v.as_bool()).unwrap_or(false);

        let history_dir = std::path::Path::new(".mt5mcp_history");
        fs::create_dir_all(history_dir)?;

        let report_name = std::path::Path::new(report_dir).file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let archive_path = history_dir.join(format!("{}.tar.gz", report_name));

        // Create tarball
        let status = std::process::Command::new("tar")
            .args(["-czf", &archive_path.to_string_lossy(), "-C", 
                   std::path::Path::new(report_dir).parent().unwrap().to_str().unwrap(),
                   report_name])
            .status()?;

        if delete_after && status.success() {
            fs::remove_dir_all(report_dir)?;
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": status.success(),
                "archive_path": archive_path.to_string_lossy(),
                "deleted": delete_after && status.success(),
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_archive_all_reports(&self, args: &Value) -> Result<Value> {
        let keep_last = args.get("keep_last").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let reports_dir = self.config.reports_dir();
        let history_dir = std::path::Path::new(".mt5mcp_history");
        fs::create_dir_all(history_dir)?;

        let mut archived = 0;

        if let Ok(entries) = fs::read_dir(&reports_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by(|a, b| {
                b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
                    .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH))
            });

            for entry in entries.into_iter().skip(keep_last) {
                let path = entry.path();
                if path.is_dir() && !path.to_string_lossy().ends_with("_opt") {
                    let report_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
                    let archive_path = history_dir.join(format!("{}.tar.gz", report_name));

                    let _ = std::process::Command::new("tar")
                        .args(["-czf", &archive_path.to_string_lossy(), "-C", 
                               path.parent().unwrap().to_str().unwrap(), report_name])
                        .status();

                    let _ = fs::remove_dir_all(&path);
                    archived += 1;
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "archived": archived,
                "kept": keep_last,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_promote_to_baseline(&self, args: &Value) -> Result<Value> {
        let report_dir = args.get("report_dir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

        let metrics_path = std::path::Path::new(report_dir).join("metrics.json");
        let baseline_path = std::path::Path::new("config/baseline.json");

        fs::copy(&metrics_path, &baseline_path)?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "baseline_file": baseline_path.to_string_lossy(),
                "source": metrics_path.to_string_lossy(),
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_get_history(&self, args: &Value) -> Result<Value> {
        let _limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        let history_dir = std::path::Path::new(".mt5mcp_history");
        let mut history = Vec::new();

        if history_dir.exists() {
            for entry in fs::read_dir(history_dir)? {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().map(|e| e == "tar.gz").unwrap_or(false) {
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        let metadata = entry.metadata()?;
                        let modified = metadata.modified()?;
                        let size = metadata.len();

                        history.push(json!({
                            "name": name,
                            "path": path.to_string_lossy(),
                            "size": size,
                            "archived_at": modified.elapsed().map(|e| e.as_secs()).unwrap_or(0),
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "total_archived": history.len(),
                "history": history,
            }).to_string() }],
            "isError": false
        }))
    }

    async fn handle_annotate_history(&self, args: &Value) -> Result<Value> {
        let report_name = args.get("report_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("report_name is required"))?;

        let note = args.get("note")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let notes_path = std::path::Path::new(".mt5mcp_history").join("notes.json");
        
        let mut notes: serde_json::Map<String, Value> = if notes_path.exists() {
            serde_json::from_str(&fs::read_to_string(&notes_path)?)?
        } else {
            serde_json::Map::new()
        };

        notes.insert(report_name.to_string(), json!(note));
        fs::write(&notes_path, serde_json::to_string_pretty(&notes)?)?;

        Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": true,
                "report": report_name,
                "note": note,
            }).to_string() }],
            "isError": false
        }))
    }
}
