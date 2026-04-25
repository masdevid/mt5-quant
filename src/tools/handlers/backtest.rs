use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::models::Config;
use crate::models::report::BacktestJob;
use crate::pipeline::backtest::{BacktestParams, BacktestPipeline};

/// Pre-flight check result for backtest readiness
#[derive(Debug)]
struct BacktestPreflight {
    account: Option<crate::models::CurrentAccount>,
    available_symbols: Vec<String>,
    ea_exists: bool,
    server: Option<String>,
}

impl BacktestPreflight {
    fn check(config: &Config, expert: &str) -> Self {
        let account = config.current_account();
        let server = account.as_ref().map(|a| a.server.clone());
        let available_symbols = config.discover_symbols_for_active_account();
        
        let ea_exists = if let Some(experts_dir) = &config.experts_dir {
            let mq5_path = std::path::Path::new(experts_dir).join(format!("{}.mq5", expert));
            let ex5_path = std::path::Path::new(experts_dir).join(format!("{}.ex5", expert));
            let subdir_mq5 = std::path::Path::new(experts_dir).join(expert).join(format!("{}.mq5", expert));
            mq5_path.exists() || ex5_path.exists() || subdir_mq5.exists()
        } else {
            false
        };
        
        Self {
            account,
            available_symbols,
            ea_exists,
            server,
        }
    }
    
    #[allow(dead_code)]
    fn is_ready(&self) -> bool {
        self.account.is_some() && !self.available_symbols.is_empty() && self.ea_exists
    }
}

pub async fn handle_run_backtest(config: &Config, args: &Value) -> Result<Value> {
    let expert = args.get("expert")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

    // Run pre-flight check
    let preflight = BacktestPreflight::check(config, expert);
    
    // Get active account context for error messages
    let active_server = preflight.server.as_deref().unwrap_or("unknown");
    let active_login = preflight.account.as_ref().map(|a| a.login.as_str()).unwrap_or("unknown");
    
    // Check account session first
    if preflight.account.is_none() {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "error": "No active MT5 account session detected.",
                "hint": "Open MT5 and login to your trading account before running backtests.",
                "pre_check": "account_missing"
            }).to_string() }],
            "isError": true
        }));
    }
    
    // Symbol pre-flight with account context
    let requested_symbol = args.get("symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // No tester data at all → hard fail with helpful context
    let no_symbols_error = || json!({
        "content": [{ "type": "text", "text": json!({
            "error": format!("No symbols available for backtesting on server '{}'.", active_server),
            "account": { "login": active_login, "server": active_server },
            "hint": "Open MT5 → View → Strategy Tester → download history for at least one symbol.",
            "pre_check": "no_symbols"
        }).to_string() }],
        "isError": true
    });

    let symbol: String = if requested_symbol.is_empty() {
        // No symbol requested — use config default or first available tester symbol.
        let candidate = config.backtest_symbol.as_deref().unwrap_or("");
        if candidate.is_empty() {
            preflight.available_symbols.first()
                .cloned()
                .ok_or(()
                ).unwrap_or_else(|_| return String::new())
        } else {
            match Config::resolve_symbol(candidate, &preflight.available_symbols) {
                Some(resolved) => {
                    if resolved != candidate {
                        tracing::warn!(
                            "Config symbol '{}' not in tester data for '{}'; using '{}' instead",
                            candidate, active_server, resolved
                        );
                    }
                    resolved.to_string()
                }
                None => {
                    // Config default has no tester data — pick first available
                    preflight.available_symbols.first()
                        .cloned()
                        .unwrap_or_default()
                }
            }
        }
    } else {
        // Caller specified a symbol — resolve it against actual tester data.
        if preflight.available_symbols.is_empty() {
            return Ok(no_symbols_error());
        }
        match Config::resolve_symbol(requested_symbol, &preflight.available_symbols) {
            Some(resolved) if resolved == requested_symbol => {
                // Exact match — use as-is
                resolved.to_string()
            }
            Some(resolved) => {
                // Fuzzy match — proceed with the corrected symbol, surface the substitution
                tracing::warn!(
                    "Symbol '{}' not in tester data for '{}'; substituting '{}'",
                    requested_symbol, active_server, resolved
                );
                resolved.to_string()
            }
            None => {
                // No match at all — fail with full context so the caller can act
                return Ok(json!({
                    "content": [{ "type": "text", "text": json!({
                        "error": format!(
                            "Symbol '{}' has no tester data on server '{}' and no close match was found.",
                            requested_symbol, active_server
                        ),
                        "account": { "login": active_login, "server": active_server },
                        "requested_symbol": requested_symbol,
                        "available_symbols": preflight.available_symbols,
                        "hint": "The tester data for this symbol hasn't been downloaded yet. \
                                 Open MT5 → Strategy Tester → select the symbol and click Download.",
                        "pre_check": "symbol_not_available"
                    }).to_string() }],
                    "isError": true
                }));
            }
        }
    };

    // Guard against the empty-string edge case (no symbols at all)
    if symbol.is_empty() {
        return Ok(no_symbols_error());
    }
    
    // EA existence check with context
    if !preflight.ea_exists {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "error": format!("EA '{}' not found in Experts directory.", expert),
                "hint": "Use search_experts or list_experts to find available EAs.",
                "pre_check": "ea_not_found"
            }).to_string() }],
            "isError": true
        }));
    }

    // Date defaulting: current month
    let (from_date, to_date) = {
        let f = args.get("from_date").and_then(|v| v.as_str()).unwrap_or("");
        let t = args.get("to_date").and_then(|v| v.as_str()).unwrap_or("");
        if f.is_empty() || t.is_empty() {
            super::current_month()
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
        shutdown: args.get("shutdown").and_then(|v| v.as_bool()).unwrap_or(true),
        kill_existing: args.get("kill_existing").and_then(|v| v.as_bool()).unwrap_or(false),
        timeout: args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(900),
        gui: args.get("gui").and_then(|v| v.as_bool()).unwrap_or(false),
        startup_delay_secs: args.get("startup_delay_secs").and_then(|v| v.as_u64()).unwrap_or(0),
        inactivity_kill_secs: args.get("inactivity_kill_secs").and_then(|v| v.as_u64()),
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

pub async fn handle_run_backtest_quick(handler: &crate::tools::handlers::ToolHandler, args: &Value) -> Result<Value> {
    // Quick backtest: skip compile, clean → launch → background monitor → return job.
    // Uses the fire-and-forget launch path so the MCP response is returned immediately
    // and the result is available via get_backtest_status / get_latest_report once done.
    // (The synchronous blocking path exceeds MCP request timeouts for any backtest >2 min.)
    let mut args = args.clone();
    if let Some(obj) = args.as_object_mut() {
        obj.insert("skip_compile".to_string(), json!(true));
    }
    handle_launch_backtest(handler, &args).await
}

pub async fn handle_run_backtest_only(handler: &crate::tools::handlers::ToolHandler, args: &Value) -> Result<Value> {
    // Backtest only: skip compile and analyze — launch and return job immediately.
    let mut args = args.clone();
    if let Some(obj) = args.as_object_mut() {
        obj.insert("skip_compile".to_string(), json!(true));
        obj.insert("skip_analyze".to_string(), json!(true));
    }
    handle_launch_backtest(handler, &args).await
}

pub async fn handle_launch_backtest(handler: &crate::tools::handlers::ToolHandler, args: &Value) -> Result<Value> {
    let expert = args.get("expert")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

    // Run pre-flight check
    let preflight = BacktestPreflight::check(&handler.config, expert);
    
    // Check account session
    if preflight.account.is_none() {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "error": "No active MT5 account session detected.",
                "hint": "Open MT5 and login to your trading account before running backtests."
            }).to_string() }],
            "isError": true
        }));
    }
    
    // Get symbol — resolve against actual tester data (same logic as handle_run_backtest)
    let active_server = preflight.server.as_deref().unwrap_or("unknown");
    let requested_symbol = args.get("symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let symbol: String = if requested_symbol.is_empty() {
        let candidate = handler.config.backtest_symbol.as_deref().unwrap_or("");
        if candidate.is_empty() {
            preflight.available_symbols.first().cloned().unwrap_or_default()
        } else {
            Config::resolve_symbol(candidate, &preflight.available_symbols)
                .map(|s| s.to_string())
                .or_else(|| preflight.available_symbols.first().cloned())
                .unwrap_or_else(|| candidate.to_string())
        }
    } else {
        match Config::resolve_symbol(requested_symbol, &preflight.available_symbols) {
            Some(resolved) => {
                if resolved != requested_symbol {
                    tracing::warn!(
                        "launch_backtest: symbol '{}' not in tester data for '{}'; using '{}'",
                        requested_symbol, active_server, resolved
                    );
                }
                resolved.to_string()
            }
            None if preflight.available_symbols.is_empty() => requested_symbol.to_string(),
            None => {
                return Ok(json!({
                    "content": [{ "type": "text", "text": json!({
                        "error": format!(
                            "Symbol '{}' has no tester data on server '{}' and no close match was found.",
                            requested_symbol, active_server
                        ),
                        "requested_symbol": requested_symbol,
                        "available_symbols": preflight.available_symbols,
                        "hint": "Open MT5 → Strategy Tester → select the symbol and click Download.",
                        "pre_check": "symbol_not_available"
                    }).to_string() }],
                    "isError": true
                }));
            }
        }
    };
    
    // EA existence check
    if !preflight.ea_exists {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "error": format!("EA '{}' not found in Experts directory.", expert),
                "hint": "Use search_experts or list_experts to find available EAs."
            }).to_string() }],
            "isError": true
        }));
    }

    // Date defaulting
    let (from_date, to_date) = {
        let f = args.get("from_date").and_then(|v| v.as_str()).unwrap_or("");
        let t = args.get("to_date").and_then(|v| v.as_str()).unwrap_or("");
        if f.is_empty() || t.is_empty() {
            super::current_month()
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
        skip_analyze: true, // Not needed for launch mode
        deep_analyze: false,
        // ShutdownTerminal=1 (default): MT5 writes the HTML report and exits cleanly.
        // The background monitor's post-exit scan finds the report within 10s.
        // The inactivity watchdog is intentionally skipped when shutdown=true so it
        // doesn't race with MT5's report write right before natural exit.
        shutdown: args.get("shutdown").and_then(|v| v.as_bool()).unwrap_or(true),
        kill_existing: false,
        timeout: args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(900),
        gui: args.get("gui").and_then(|v| v.as_bool()).unwrap_or(false),
        startup_delay_secs: args.get("startup_delay_secs").and_then(|v| v.as_u64()).unwrap_or(0),
        inactivity_kill_secs: args.get("inactivity_kill_secs").and_then(|v| v.as_u64()),
    };

    let pipeline = if let Some(ref callback) = handler.notification_callback {
        BacktestPipeline::with_notification_callback(handler.config.clone(), callback.clone())
    } else {
        BacktestPipeline::new(handler.config.clone())
    };
    let job = pipeline.launch_backtest(params).await?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "message": "Backtest launched successfully. Use get_backtest_status to poll for completion.",
            "report_id": job.report_id,
            "report_dir": job.report_dir,
            "expert": job.expert,
            "symbol": job.symbol,
            "timeframe": job.timeframe,
            "launched_at": job.launched_at,
            "timeout_seconds": job.timeout_seconds,
            "poll_hint": "Call get_backtest_status with report_dir to check progress"
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_backtest_status(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("latest");

    let report_path = Path::new(report_dir);
    let progress_file = report_path.join("progress.log");
    let job_file = report_path.join("job.json");
    
    // Load job info if available
    let job: Option<BacktestJob> = if job_file.exists() {
        fs::read_to_string(&job_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    } else {
        None
    };
    
    // Check progress log for stage
    let (stage, progress_lines) = if progress_file.exists() {
        if let Ok(content) = fs::read_to_string(&progress_file) {
            let lines: Vec<&str> = content.lines().collect();
            let last_stage = lines.last()
                .and_then(|l| l.split_whitespace().next())
                .unwrap_or("UNKNOWN");
            (last_stage.to_string(), lines.len())
        } else {
            ("UNKNOWN".to_string(), 0)
        }
    } else {
        ("NOT_STARTED".to_string(), 0)
    };
    
    // Check if MT5 is running
    let mt5_running = is_mt5_running();
    
    // Check if report file exists (still on disk — it's deleted after extraction)
    let report_found = job.as_ref()
        .map(|j| Path::new(&j.expected_report_path).exists())
        .unwrap_or(false);

    // Check for completed artifacts
    let metrics_exists = report_path.join("metrics.json").exists();

    // Read the authoritative status written by the background monitor into job.json.
    // This avoids false "failed" when the HTML was extracted+deleted (report_found=false)
    // or when journal extraction ran instead of HTML extraction.
    let job_status = job.as_ref()
        .and_then(|j| j.status.as_deref())
        .unwrap_or("");
    let monitor_says_complete = matches!(job_status, "completed" | "completed_no_html");
    let monitor_says_failed   = matches!(job_status, "failed" | "timeout" | "timeout_inactive");

    let is_complete = monitor_says_complete
        || stage == "DONE"
        || (report_found && metrics_exists);

    // Calculate elapsed time if job exists
    let elapsed_seconds = job.as_ref()
        .and_then(|j| {
            chrono::DateTime::parse_from_rfc3339(&j.launched_at)
                .ok()
                .map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds())
        })
        .unwrap_or(0);

    // Determine status message — trust monitor's job.json first
    let status_msg = if is_complete {
        "completed"
    } else if monitor_says_failed {
        if job_status == "timeout" || job_status == "timeout_inactive" { "timeout" } else { "failed" }
    } else if stage == "BACKTEST" && mt5_running {
        "running"
    } else if stage == "BACKTEST" && !mt5_running {
        "failed"
    } else if progress_lines > 0 {
        "in_progress"
    } else {
        "not_started"
    };

    let message = if is_complete {
        if job_status == "completed_no_html" {
            "Backtest completed (extracted from tester journal — no HTML report)"
        } else {
            "Backtest completed successfully"
        }
    } else if stage == "BACKTEST" && mt5_running {
        "MT5 is running the backtest"
    } else if stage == "BACKTEST" && !mt5_running {
        "MT5 process exited — report not yet found"
    } else {
        &format!("Backtest is at stage: {}", stage)
    };

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "report_dir": report_dir,
            "status": status_msg,
            "stage": stage,
            "is_complete": is_complete,
            "mt5_running": mt5_running,
            "report_found": report_found,
            "metrics_extracted": metrics_exists,
            "elapsed_seconds": elapsed_seconds,
            "message": message,
            "job": job.map(|j| {
                json!({
                    "report_id": j.report_id,
                    "expert": j.expert,
                    "symbol": j.symbol,
                    "timeframe": j.timeframe,
                    "launched_at": j.launched_at,
                    "timeout_seconds": j.timeout_seconds
                })
            })
        }).to_string() }],
        "isError": false
    }))
}

/// Check if MT5 is currently running
fn is_mt5_running() -> bool {
    let patterns = if cfg!(target_os = "macos") {
        vec!["MetaTrader 5\\.app", "terminal64\\.exe"]
    } else {
        vec!["terminal64\\.exe", "metatrader"]
    };
    
    patterns.iter().any(|pat| {
        Command::new("pgrep")
            .args(["-f", pat])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Read the active tester agent log for live/post-test deal inspection.
/// Returns the last N lines and a parsed deal summary.
pub async fn handle_get_tester_log(config: &Config, args: &Value) -> Result<Value> {
    use crate::pipeline::backtest::BacktestPipeline;

    let tail_lines = args.get("tail_lines").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let log_path = match BacktestPipeline::find_active_tester_agent_log(config) {
        Some(p) => p,
        None => {
            return Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "error": "No tester agent log found for today.",
                    "hint": "Run a backtest first. The log appears after the tester starts."
                }).to_string() }],
                "isError": true
            }));
        }
    };

    let lines = match BacktestPipeline::read_tester_agent_log(&log_path) {
        Some(l) => l,
        None => {
            return Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "error": format!("Could not read log at {}", log_path.display())
                }).to_string() }],
                "isError": true
            }));
        }
    };

    let (deals, final_balance_pips, progress) = BacktestPipeline::parse_journal_deals(&lines);

    // Collect tail lines
    let tail: Vec<&str> = lines.iter()
        .rev()
        .take(tail_lines)
        .rev()
        .map(|s| s.as_str())
        .collect();

    // Detect last sim timestamp for progress estimation
    let last_sim_time = lines.iter().rev()
        .find_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            // Format: XX  0  HH:MM:SS.mmm  Core NN  YYYY.MM.DD HH:MM:SS ...
            if parts.len() >= 6 {
                let date = parts[4];
                let time = parts[5];
                if date.contains('.') && time.contains(':') {
                    return Some(format!("{} {}", date, time));
                }
            }
            None
        })
        .unwrap_or_default();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "log_path": log_path.to_string_lossy(),
            "total_lines": lines.len(),
            "deals_found": deals.len(),
            "final_balance_pips": final_balance_pips,
            "progress": progress,
            "last_sim_time": last_sim_time,
            "is_complete": !progress.is_empty(),
            "tail_lines": tail,
            "deals_summary": deals.iter().map(|d| json!({
                "deal": d.deal,
                "time": d.time,
                "type": d.deal_type,
                "volume": d.volume,
                "price": d.price,
                "symbol": d.symbol,
            })).collect::<Vec<_>>()
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
