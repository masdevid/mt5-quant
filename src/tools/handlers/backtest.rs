use anyhow::Result;
use chrono::Datelike;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::time::Duration;
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
        // On Wine/macOS, ShutdownTerminal=1 does NOT reliably cause terminal64.exe to exit.
        // When inactivity_kill_secs is set, the monitor waits that many seconds after the
        // tester log goes quiet, then polls for the HTML report for 30 s, then kills MT5.
        // If no inactivity_kill_secs is given (default None → disabled), the monitor relies
        // solely on timeout (900 s) or natural MT5 exit for completion detection.
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

pub async fn handle_run_rolling_backtest(config: &Config, args: &Value) -> Result<Value> {
    let expert = args.get("expert")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

    let weeks_count = args.get("weeks").and_then(|v| v.as_u64()).unwrap_or(4) as i64;
    if weeks_count < 1 || weeks_count > 52 {
        return Ok(json!({
            "content": [{ "type": "text", "text": "weeks must be between 1 and 52".to_string() }],
            "isError": true
        }));
    }

    // Calculate weekly date ranges
    let from_arg = args.get("from_date").and_then(|v| v.as_str()).unwrap_or("");
    let to_arg = args.get("to_date").and_then(|v| v.as_str()).unwrap_or("");

    let weeks: Vec<(String, String, String)> = if !from_arg.is_empty() && !to_arg.is_empty() {
        let start = chrono::NaiveDate::parse_from_str(from_arg, "%Y.%m.%d")
            .map_err(|e| anyhow::anyhow!("Invalid from_date '{}': {}", from_arg, e))?;
        let end = chrono::NaiveDate::parse_from_str(to_arg, "%Y.%m.%d")
            .map_err(|e| anyhow::anyhow!("Invalid to_date '{}': {}", to_arg, e))?;
        if end <= start {
            return Ok(json!({
                "content": [{ "type": "text", "text": "to_date must be after from_date".to_string() }],
                "isError": true
            }));
        }
        let mut weeks = Vec::new();
        let mut current = start;
        let mut week_idx = 0;
        while current < end && weeks.len() < weeks_count as usize {
            week_idx += 1;
            let week_end = if current + chrono::Duration::days(7) >= end {
                end
            } else {
                let days_to_sun = 6 - current.weekday().num_days_from_monday();
                let week_end_raw = current + chrono::Duration::days(days_to_sun as i64);
                std::cmp::min(week_end_raw, end)
            };
            let label = format!("Week {}", week_idx);
            weeks.push((label, current.format("%Y.%m.%d").to_string(), week_end.format("%Y.%m.%d").to_string()));
            current = week_end + chrono::Duration::days(1);
        }
        weeks
    } else {
        let now = chrono::Utc::now().date_naive();
        let days_from_sun = now.weekday().num_days_from_sunday();
        let last_sun = now - chrono::Duration::days(days_from_sun as i64);
        let mut weeks = Vec::new();
        for i in 0..weeks_count {
            let i = weeks_count - 1 - i;
            let week_end = last_sun - chrono::Duration::days(i * 7);
            let week_start = week_end - chrono::Duration::days(6);
            let label = format!(
                "{} - {}",
                week_start.format("%b %d"),
                week_end.format("%b %d")
            );
            weeks.push((label, week_start.format("%Y.%m.%d").to_string(), week_end.format("%Y.%m.%d").to_string()));
        }
        weeks
    };

    let symbol = args.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let timeframe = args.get("timeframe").and_then(|v| v.as_str()).unwrap_or("M5").to_string();
    let deposit = args.get("deposit").and_then(|v| v.as_u64()).unwrap_or(10000) as u32;
    let model = args.get("model").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let leverage: u32 = args.get("leverage").and_then(|v| v.as_u64()).unwrap_or(500) as u32;
    let set_file = args.get("set_file").and_then(|v| v.as_str()).map(|s| s.to_string());
    let deep_analyze = args.get("deep").and_then(|v| v.as_bool()).unwrap_or(false);
    let shutdown = args.get("shutdown").and_then(|v| v.as_bool()).unwrap_or(true);
    let kill_existing = args.get("kill_existing").and_then(|v| v.as_bool()).unwrap_or(true);
    let timeout = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(900);
    let gui = args.get("gui").and_then(|v| v.as_bool()).unwrap_or(false);
    let startup_delay_secs = args.get("startup_delay_secs").and_then(|v| v.as_u64()).unwrap_or(0);
    let skip_compile_first = args.get("skip_compile").and_then(|v| v.as_bool()).unwrap_or(false);

    // Create a rolling job report dir for status tracking
    let report_id = format!(
        "ROLLING_{}_{}_{}",
        expert,
        weeks.first().map(|w| &w.1).unwrap_or(&"unknown".to_string()),
        weeks.last().map(|w| &w.2).unwrap_or(&"unknown".to_string()),
    );
    let report_dir = config.reports_dir().join(&report_id);
    fs::create_dir_all(&report_dir)?;

    let job_path = report_dir.join("job.json");
    let job = BacktestJob {
        report_id: report_id.clone(),
        report_dir: report_dir.to_string_lossy().to_string(),
        expert: expert.to_string(),
        symbol: symbol.clone(),
        timeframe: timeframe.clone(),
        mt5_pid: None,
        expected_report_path: String::new(),
        timeout_seconds: timeout * weeks.len() as u64,
        launched_at: chrono::Utc::now().to_rfc3339(),
        status: Some("launched".to_string()),
    };
    fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;

    // Save the weekly schedule for status polling
    let weeks_path = report_dir.join("weeks.json");
    fs::write(&weeks_path, serde_json::to_string_pretty(&json!({
        "weeks": weeks.iter().map(|(l, f, t)| json!({"label": l, "from_date": f, "to_date": t})).collect::<Vec<_>>()
    }))?)?;

    // Spawn background task to run all weeks sequentially
    let config_clone = config.clone();
    let report_dir_clone = report_dir.clone();
    let expert_clone = expert.to_string();
    let symbol_clone = symbol;
    let timeframe_clone = timeframe;
    let set_file_clone = set_file;
    let weeks_clone = weeks.clone();

    tokio::spawn(async move {
        if let Err(e) = run_rolling_weeks_sequential(
            config_clone,
            report_dir_clone,
            expert_clone,
            symbol_clone,
            timeframe_clone,
            deposit,
            model,
            leverage,
            set_file_clone,
            deep_analyze,
            shutdown,
            kill_existing,
            timeout,
            gui,
            startup_delay_secs,
            skip_compile_first,
            weeks_clone,
        ).await {
            tracing::error!("Rolling backtest failed: {}", e);
        }
    });

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "message": format!("Rolling backtest launched with {} weeks. Use get_backtest_status to poll for completion.", weeks.len()),
            "report_id": report_id,
            "report_dir": report_dir.to_string_lossy(),
            "expert": expert,
            "weeks": weeks.iter().map(|(l, f, t)| json!({"label": l, "from_date": f, "to_date": t})).collect::<Vec<_>>(),
            "poll_hint": "Call get_backtest_status with report_dir to check progress"
        }).to_string() }],
        "isError": false
    }))
}

async fn run_rolling_weeks_sequential(
    config: Config,
    report_dir: PathBuf,
    expert: String,
    symbol: String,
    timeframe: String,
    deposit: u32,
    model: u8,
    leverage: u32,
    set_file: Option<String>,
    deep_analyze: bool,
    _shutdown: bool,
    _kill_existing: bool,
    timeout: u64,
    gui: bool,
    startup_delay_secs: u64,
    skip_compile_first: bool,
    weeks: Vec<(String, String, String)>,
) -> Result<()> {
    let pipeline = if cfg!(debug_assertions) {
        BacktestPipeline::new(config.clone())
    } else {
        BacktestPipeline::new(config.clone())
    };
    let progress_log = report_dir.join("progress.log");

    // Clear stale results
    let results_path = report_dir.join("rolling_results.json");
    let _ = fs::remove_file(&results_path);

    let mut week_results = Vec::new();
    let mut total_net_profit: f64 = 0.0;
    let mut max_drawdown: f64 = 0.0;
    let mut total_trades: u64 = 0;

    // Kill MT5/wineserver before starting
    for pat in &["MetaTrader 5.app", "terminal64.exe", "wineserver"] {
        let _ = Command::new("pkill").args(["-KILL", "-f", pat]).output();
    }
    tokio::time::sleep(Duration::from_secs(5)).await;

    for (idx, (label, from_date, to_date)) in weeks.iter().enumerate() {
        let skip_compile = if idx == 0 { skip_compile_first } else { true };

        fs::write(&progress_log, format!("WEEK {}: {} ({} -> {})\n", idx + 1, label, from_date, to_date))
            .unwrap_or(());

        tracing::info!("Rolling week {}/{}: {} ({} -> {})", idx + 1, weeks.len(), label, from_date, to_date);

        // Clean slate before each week
        for pat in &["MetaTrader 5.app", "terminal64.exe", "wineserver"] {
            let _ = Command::new("pkill").args(["-KILL", "-f", pat]).output();
        }
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Fire-and-forget launch (handles journal fallback if HTML not produced)
        let params = BacktestParams {
            expert: expert.clone(),
            symbol: symbol.clone(),
            from_date: from_date.clone(),
            to_date: to_date.clone(),
            timeframe: timeframe.clone(),
            deposit,
            model,
            leverage,
            set_file: set_file.clone(),
            skip_compile,
            skip_clean: false,
            skip_analyze: false,
            deep_analyze,
            shutdown: true,
            kill_existing: false,
            timeout,
            gui,
            startup_delay_secs,
            inactivity_kill_secs: None,
        };

        let job = match pipeline.launch_backtest(params).await {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("Rolling week {}/{} launch failed: {}", idx + 1, weeks.len(), e);
                week_results.push(json!({
                    "label": label, "from_date": from_date, "to_date": to_date,
                    "success": false, "error": format!("launch failed: {}", e)
                }));
                continue;
            }
        };

        // Poll for completion (up to timeout)
        let week_report_dir = Path::new(&job.report_dir);
        let poll_start = std::time::Instant::now();
        let max_wait = Duration::from_secs(timeout);
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Check for metrics.json (written after extraction, including journal fallback)
            let metrics_path = week_report_dir.join("metrics.json");
            if metrics_path.exists() { break; }

            let jp = week_report_dir.join("job.json");
            if let Ok(content) = fs::read_to_string(&jp) {
                if let Ok(j) = serde_json::from_str::<BacktestJob>(&content) {
                    if let Some(ref s) = j.status {
                        if s == "completed" || s == "completed_no_html" { break; }
                        if s == "failed" || s == "timeout" || s == "timeout_inactive" {
                            tracing::warn!("Rolling week {}/{} background status: {}", idx + 1, weeks.len(), s);
                            break;
                        }
                    }
                }
            }

            if poll_start.elapsed() > max_wait {
                tracing::warn!("Rolling week {}/{} timed out after {}s", idx + 1, weeks.len(), timeout);
                break;
            }
        }

        // Wait a moment for extraction to finish
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Read results
        let metrics_path = week_report_dir.join("metrics.json");
        if let Ok(content) = fs::read_to_string(&metrics_path) {
            if let Ok(metrics) = serde_json::from_str::<serde_json::Value>(&content) {
                let np = metrics.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dd = metrics.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let trades = metrics.get("total_trades").and_then(|v| v.as_u64()).unwrap_or(0);
                let pf = metrics.get("profit_factor").and_then(|v| v.as_f64()).unwrap_or(0.0);

                total_net_profit += np;
                if dd > max_drawdown { max_drawdown = dd; }
                total_trades += trades;

                tracing::info!("Rolling week {}/{} done: profit={:.2}, dd={:.1}%", idx + 1, weeks.len(), np, dd);

                week_results.push(json!({
                    "label": label, "from_date": from_date, "to_date": to_date,
                    "success": true, "net_profit": np, "max_dd_pct": dd,
                    "total_trades": trades, "profit_factor": pf,
                    "report_dir": job.report_dir
                }));
                continue;
            }
        }

        // No metrics found
        week_results.push(json!({
            "label": label, "from_date": from_date, "to_date": to_date,
            "success": false, "error": "No metrics extracted"
        }));
    }

    // Write final results
    let summary = json!({
        "success": true,
        "weeks_run": week_results.len(),
        "summary": {
            "total_net_profit": total_net_profit,
            "max_drawdown_pct": max_drawdown,
            "total_trades": total_trades,
        },
        "weekly_results": week_results
    });
    if let Ok(json_str) = serde_json::to_string_pretty(&summary) {
        let _ = fs::write(&results_path, &json_str);
    }

    // Update job status
    let job_path = report_dir.join("job.json");
    if let Ok(content) = fs::read_to_string(&job_path) {
        if let Ok(mut job) = serde_json::from_str::<BacktestJob>(&content) {
            job.status = Some("completed".to_string());
            if let Ok(json_str) = serde_json::to_string_pretty(&job) {
                let _ = fs::write(&job_path, json_str);
            }
        }
    }

    fs::write(&progress_log, "DONE\n").unwrap_or(());
    tracing::info!("Rolling backtest completed: {} weeks, profit={:.2}, max_dd={:.1}%", week_results.len(), total_net_profit, max_drawdown);
    Ok(())
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
