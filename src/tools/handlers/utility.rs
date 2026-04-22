use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use crate::models::Config;
use crate::storage::ReportDb;

/// Validate that `user_path` resolves to a location within `allowed_base`.
/// Returns the canonicalized absolute path on success.
fn safe_output_path(user_path: &str, allowed_base: &Path) -> Result<PathBuf> {
    // Resolve the base first (must already exist).
    let base = allowed_base.canonicalize()
        .with_context(|| format!("allowed base directory does not exist: {}", allowed_base.display()))?;

    // Build the candidate path. If the user supplied an absolute path we use it
    // as-is; relative paths are joined onto the base so they stay inside it.
    let candidate = {
        let p = Path::new(user_path);
        if p.is_absolute() { p.to_path_buf() } else { base.join(p) }
    };

    // Canonicalize the parent directory (the file itself need not exist yet).
    let parent = candidate.parent()
        .ok_or_else(|| anyhow::anyhow!("output_path has no parent directory"))?;
    let canonical_parent = parent.canonicalize()
        .with_context(|| format!("output_path parent directory does not exist: {}", parent.display()))?;
    let canonical = canonical_parent.join(
        candidate.file_name()
            .ok_or_else(|| anyhow::anyhow!("output_path must include a filename"))?,
    );

    if !canonical.starts_with(&base) {
        return Err(anyhow::anyhow!(
            "output_path '{}' is outside the allowed directory '{}'",
            user_path, base.display()
        ));
    }
    Ok(canonical)
}

/// Check if symbol has sufficient data for date range
pub async fn handle_check_symbol_data_status(config: &Config, args: &Value) -> Result<Value> {
    let symbol = args.get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("symbol is required"))?;
    
    let from_date = args.get("from_date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("from_date is required"))?;
    
    let to_date = args.get("to_date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("to_date is required"))?;
    
    // Get active account to determine which server to check
    let current_account = config.current_account();
    let server = current_account.as_ref().map(|a| a.server.as_str()).unwrap_or("");
    
    // Check if symbol exists in available symbols
    let available_symbols = config.discover_symbols_for_active_account();
    let symbol_available = available_symbols.contains(&symbol.to_string());
    
    if !symbol_available {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "symbol": symbol,
                "has_sufficient_data": false,
                "error": format!("Symbol '{}' not available for server '{}'", symbol, server),
                "available_symbols": available_symbols,
                "suggestion": "Use get_active_account to see available symbols for this account"
            }).to_string() }],
            "isError": false
        }));
    }
    
    // Try to find hcc files to determine actual data range
    let mt5_dir = config.mt5_dir();
    let mut data_range_start = None;
    let mut data_range_end = None;
    let mut bars_count = 0;
    
    if let Some(mt5_path) = mt5_dir {
        let bases_dir = mt5_path.join("Bases");
        if bases_dir.exists() {
            // Look through servers for this symbol
            for server_entry in fs::read_dir(&bases_dir)?.flatten() {
                let server_name = server_entry.file_name().to_string_lossy().to_string();
                if server.is_empty() || server_name == server {
                    let symbol_dir = server_entry.path().join("history").join(symbol);
                    if symbol_dir.exists() {
                        // Count hcc files and get date range
                        for entry in fs::read_dir(&symbol_dir)?.flatten() {
                            if let Some(ext) = entry.path().extension() {
                                if ext == "hcc" {
                                    bars_count += 1;
                                    if let Some(fname) = entry.file_name().to_str() {
                                        // Parse year from filename (e.g., "2024.hcc")
                                        if let Ok(year) = fname.trim_end_matches(".hcc").parse::<i32>() {
                                            if data_range_start.is_none() || year < data_range_start.unwrap() {
                                                data_range_start = Some(year);
                                            }
                                            if data_range_end.is_none() || year > data_range_end.unwrap() {
                                                data_range_end = Some(year);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Parse requested date range
    let parse_date = |date_str: &str| -> Option<(i32, i32, i32)> {
        let parts: Vec<&str> = date_str.split('.').collect();
        if parts.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (parts[0].parse(), parts[1].parse(), parts[2].parse()) {
                return Some((y, m, d));
            }
        }
        None
    };
    
    let req_from = parse_date(from_date);
    let req_to = parse_date(to_date);
    
    let mut has_sufficient = true;
    let mut warnings = Vec::new();
    
    if let (Some(req_year), Some(start_year), Some(end_year)) = (req_from.map(|d| d.0), data_range_start, data_range_end) {
        if req_year < start_year {
            has_sufficient = false;
            warnings.push(format!("Requested start year {} is before available data start {}", req_year, start_year));
        }
        if let Some(req_to_year) = req_to.map(|d| d.0) {
            if req_to_year > end_year {
                has_sufficient = false;
                warnings.push(format!("Requested end year {} is after available data end {}", req_to_year, end_year));
            }
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "symbol": symbol,
            "server": server,
            "has_sufficient_data": has_sufficient && bars_count > 0,
            "requested_range": {
                "from": from_date,
                "to": to_date
            },
            "data_range": match (data_range_start, data_range_end) {
                (Some(s), Some(e)) => format!("{}.01.01 - {}.12.31", s, e),
                _ => "unknown".to_string()
            },
            "years_available": data_range_end.map(|e| e - data_range_start.unwrap_or(e) + 1).unwrap_or(0),
            "hcc_files_count": bars_count,
            "warnings": if warnings.is_empty() { None } else { Some(warnings) },
            "suggestion": if !has_sufficient { "Consider downloading more history in MT5 or adjusting date range" } else { "Data range is sufficient for backtest" }
        }).to_string() }],
        "isError": false
    }))
}

/// Get backtest history for EA/symbol
pub async fn handle_get_backtest_history(config: &Config, args: &Value) -> Result<Value> {
    let expert_filter = args.get("expert").and_then(|v| v.as_str());
    let symbol_filter = args.get("symbol").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    
    let reports_dir = config.reports_dir();
    let mut history = Vec::new();
    
    if reports_dir.exists() {
        for entry in fs::read_dir(&reports_dir)?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Try to read metrics.json
                let metrics_path = path.join("metrics.json");
                if let Ok(content) = fs::read_to_string(&metrics_path) {
                    if let Ok(metrics) = serde_json::from_str::<Value>(&content) {
                        let report_expert = metrics.get("expert").and_then(|v| v.as_str());
                        let report_symbol = metrics.get("symbol").and_then(|v| v.as_str());
                        
                        // Apply filters
                        if let (Some(e), Some(filter)) = (report_expert, expert_filter) {
                            if !e.contains(filter) {
                                continue;
                            }
                        }
                        if let (Some(s), Some(filter)) = (report_symbol, symbol_filter) {
                            if s != filter {
                                continue;
                            }
                        }
                        
                        // Extract key metrics
                        let summary = json!({
                            "report_dir": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                            "date": metrics.get("backtest_date").and_then(|v| v.as_str()),
                            "expert": report_expert,
                            "symbol": report_symbol,
                            "period": metrics.get("period").and_then(|v| v.as_str()),
                            "profit": metrics.get("net_profit").and_then(|v| v.as_f64()),
                            "profit_factor": metrics.get("profit_factor").and_then(|v| v.as_f64()),
                            "expected_payoff": metrics.get("expected_payoff").and_then(|v| v.as_f64()),
                            "drawdown_pct": metrics.get("drawdown_pct").and_then(|v| v.as_f64()),
                            "total_trades": metrics.get("total_trades").and_then(|v| v.as_u64()),
                            "win_rate": metrics.get("win_rate").and_then(|v| v.as_f64()),
                        });
                        
                        history.push(summary);
                    }
                }
            }
        }
    }
    
    // Sort by date (newest first)
    history.sort_by(|a, b| {
        let date_a = a.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let date_b = b.get("date").and_then(|v| v.as_str()).unwrap_or("");
        date_b.cmp(date_a)
    });
    
    // Apply limit
    let total = history.len();
    let limited: Vec<Value> = history.into_iter().take(limit).collect();
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": limited.len(),
            "total": total,
            "filters": {
                "expert": expert_filter,
                "symbol": symbol_filter
            },
            "history": limited,
            "hint": if limited.is_empty() { "No backtest history found. Run a backtest first with run_backtest." } else { "Use compare_backtests to analyze differences between results." }
        }).to_string() }],
        "isError": false
    }))
}

/// Compare multiple backtests
pub async fn handle_compare_backtests(_config: &Config, args: &Value) -> Result<Value> {
    let report_dirs = args.get("report_dirs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("report_dirs array is required"))?;
    
    if report_dirs.len() < 2 {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "error": "At least 2 report directories required for comparison"
            }).to_string() }],
            "isError": true
        }));
    }
    
    let mut comparisons = Vec::new();
    let mut base_metrics: Option<Value> = None;
    
    for dir_value in report_dirs {
        let dir = dir_value.as_str().unwrap_or("");
        let path = Path::new(dir);
        let metrics_path = path.join("metrics.json");
        
        if let Ok(content) = fs::read_to_string(&metrics_path) {
            if let Ok(metrics) = serde_json::from_str::<Value>(&content) {
                if base_metrics.is_none() {
                    base_metrics = Some(metrics.clone());
                }
                
                let summary = json!({
                    "report_dir": dir,
                    "expert": metrics.get("expert").and_then(|v| v.as_str()),
                    "symbol": metrics.get("symbol").and_then(|v| v.as_str()),
                    "net_profit": metrics.get("net_profit").and_then(|v| v.as_f64()),
                    "profit_factor": metrics.get("profit_factor").and_then(|v| v.as_f64()),
                    "drawdown_pct": metrics.get("drawdown_pct").and_then(|v| v.as_f64()),
                    "total_trades": metrics.get("total_trades").and_then(|v| v.as_u64()),
                    "win_rate": metrics.get("win_rate").and_then(|v| v.as_f64()),
                    "expected_payoff": metrics.get("expected_payoff").and_then(|v| v.as_f64()),
                    "recovery_factor": metrics.get("recovery_factor").and_then(|v| v.as_f64()),
                    "sharpe_ratio": metrics.get("sharpe_ratio").and_then(|v| v.as_f64()),
                });
                
                comparisons.push(summary);
            }
        }
    }
    
    // Calculate differences if we have base
    let mut analysis = Vec::new();
    if let Some(base) = &base_metrics {
        let base_profit = base.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let base_dd = base.get("drawdown_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let base_pf = base.get("profit_factor").and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        for (_i, comp) in comparisons.iter().enumerate().skip(1) {
            let profit = comp.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dd = comp.get("drawdown_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let pf = comp.get("profit_factor").and_then(|v| v.as_f64()).unwrap_or(0.0);
            
            analysis.push(json!({
                "compare_to": comparisons[0].get("report_dir"),
                "report": comp.get("report_dir"),
                "profit_diff": profit - base_profit,
                "profit_pct_change": if base_profit != 0.0 { ((profit - base_profit) / base_profit.abs()) * 100.0 } else { 0.0 },
                "drawdown_diff": dd - base_dd,
                "profit_factor_diff": pf - base_pf,
                "verdict": if profit > base_profit && dd <= base_dd { "better" } 
                           else if profit < base_profit { "worse" } 
                           else { "mixed" }
            }));
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": comparisons.len(),
            "comparisons": comparisons,
            "analysis": if analysis.is_empty() { None } else { Some(analysis) },
            "verdict": if comparisons.len() >= 2 {
                let profits: Vec<f64> = comparisons.iter()
                    .filter_map(|c| c.get("net_profit").and_then(|v| v.as_f64()))
                    .collect();
                let best_idx = profits.iter().enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i);
                best_idx.map(|i| format!("Best: {}", 
                    comparisons.get(i).and_then(|c| c.get("report_dir").and_then(|v| v.as_str())).unwrap_or("unknown")))
            } else { None }
        }).to_string() }],
        "isError": false
    }))
}

/// Initialize new MQL5 project
pub async fn handle_init_project(config: &Config, args: &Value) -> Result<Value> {
    let name = args.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("name is required"))?;
    
    let template = args.get("template").and_then(|v| v.as_str()).unwrap_or("basic");
    
    // Determine project directory
    let project_dir = config.project_dir.as_ref()
        .map(|p| Path::new(p).to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project directory"))?;
    
    let ea_path = project_dir.join(format!("{}.mq5", name));
    
    // Basic EA template
    let ea_template = match template {
        "scalper" => format!(r#"//+------------------------------------------------------------------+
//|                                              {}.mq5 |
//|                        Scalper EA Template                       |
//+------------------------------------------------------------------+
#property copyright "Your Name"
#property link      ""
#property version   "1.00"
#property strict

input double Lots = 0.1;
input int StopLoss = 20;      // points
input int TakeProfit = 10;    // points
input int Slippage = 3;

int OnInit() {{
    Print("{} Scalper EA initialized");
    return(INIT_SUCCEEDED);
}}

void OnDeinit(const int reason) {{
    Print("{} EA stopped, reason: ", reason);
}}

void OnTick() {{
    static datetime last_bar = 0;
    datetime current_bar = iTime(_Symbol, PERIOD_CURRENT, 0);
    
    if(current_bar == last_bar) return; // New bar only
    last_bar = current_bar;
    
    // Fast MAs crossover logic here
    double ma_fast = iMA(_Symbol, PERIOD_CURRENT, 5, 0, MODE_EMA, PRICE_CLOSE, 0);
    double ma_slow = iMA(_Symbol, PERIOD_CURRENT, 15, 0, MODE_EMA, PRICE_CLOSE, 0);
    
    if(PositionsTotal() == 0) {{
        if(ma_fast > ma_slow) {{
            // Buy signal
            // OrderSend(_Symbol, ORDER_TYPE_BUY, Lots, Ask, Slippage, Bid-StopLoss*_Point, Bid+TakeProfit*_Point);
        }}
        else if(ma_fast < ma_slow) {{
            // Sell signal
            // OrderSend(_Symbol, ORDER_TYPE_SELL, Lots, Bid, Slippage, Ask+StopLoss*_Point, Ask-TakeProfit*_Point);
        }}
    }}
}}
"#, name, name, name),
        
        "swing" => format!(r#"//+------------------------------------------------------------------+
//|                                              {}.mq5 |
//|                        Swing EA Template                         |
//+------------------------------------------------------------------+
#property copyright "Your Name"
#property link      ""
#property version   "1.00"
#property strict

input double Lots = 0.1;
input int StopLoss = 100;     // points
input int TakeProfit = 200;   // points
input int TrailingStop = 50;  // points

int OnInit() {{
    Print("{} Swing EA initialized");
    return(INIT_SUCCEEDED);
}}

void OnDeinit(const int reason) {{
    Print("{} EA stopped");
}}

void OnTick() {{
    // Daily/H4 trend following logic
    double trend_ma = iMA(_Symbol, PERIOD_D1, 50, 0, MODE_SMA, PRICE_CLOSE, 0);
    double price = iClose(_Symbol, PERIOD_CURRENT, 0);
    
    // Implementation here
}}
"#, name, name, name),
        
        "grid" => format!(r#"//+------------------------------------------------------------------+
//|                                              {}.mq5 |
//|                         Grid EA Template                         |
//+------------------------------------------------------------------+
#property copyright "Your Name"
#property link      ""
#property version   "1.00"
#property strict

input double StartLots = 0.01;
input double LotMultiplier = 1.5;
input int GridPips = 20;
input int MaxLevels = 10;

int OnInit() {{
    Print("{} Grid EA initialized - Use with extreme caution!");
    return(INIT_SUCCEEDED);
}}

void OnDeinit(const int reason) {{
    Print("{} Grid EA stopped");
}}

void OnTick() {{
    // Grid trading logic
    // WARNING: Grid strategies can lead to significant losses
}}
"#, name, name, name),
        
        _ => format!(r#"//+------------------------------------------------------------------+
//|                                              {}.mq5 |
//|                          Basic EA Template                       |
//+------------------------------------------------------------------+
#property copyright "Your Name"
#property link      ""
#property version   "1.00"
#property strict

input double Lots = 0.1;
input int StopLoss = 50;
input int TakeProfit = 50;

int OnInit() {{
    Print("{} EA initialized");
    return(INIT_SUCCEEDED);
}}

void OnDeinit(const int reason) {{
    Print("{} EA stopped");
}}

void OnTick() {{
    // Your trading logic here
    // Check for open positions, signals, etc.
}}
"#, name, name, name)
    };
    
    // Write EA file
    fs::write(&ea_path, ea_template)?;
    
    // Create README
    let readme_path = project_dir.join("README.md");
    let readme_content = format!(r#"# {}

MQL5 Expert Advisor generated by MT5-Quant

## Files
- `{}`.mq5 - Main EA source code

## Parameters
Edit the `input` variables in the source code to customize:
- `Lots` - Trading lot size
- `StopLoss` - Stop loss in points
- `TakeProfit` - Take profit in points

## Build & Test
```bash
# Compile EA
mt5-quant compile_ea expert={}

# Run backtest
mt5-quant run_backtest expert={} symbol=XAUUSDc
```
"#, name, name, name, name);
    
    fs::write(&readme_path, readme_content)?;
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "project_name": name,
            "template": template,
            "created_files": [
                ea_path.to_string_lossy().to_string(),
                readme_path.to_string_lossy().to_string()
            ],
            "hint": format!("Edit {}.mq5 to implement your strategy, then compile with compile_ea", name)
        }).to_string() }],
        "isError": false
    }))
}

/// Validate EA syntax (basic check without full compile)
pub async fn handle_validate_ea_syntax(_config: &Config, args: &Value) -> Result<Value> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;
    
    let content = fs::read_to_string(path)?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    
    // Basic syntax checks
    let lines: Vec<&str> = content.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();
        
        // Check for basic issues
        if trimmed.ends_with('{') && !trimmed.contains('}') {
            // This is just a heuristic, not a real syntax error
        }
        
        if trimmed.contains("OrderSend") && !trimmed.starts_with("//") {
            warnings.push(json!({
                "line": line_num,
                "message": "OrderSend found - ensure proper error handling",
                "severity": "warning"
            }));
        }
        
        if trimmed.contains("input") && trimmed.contains("double") && trimmed.contains("Lots") {
            if !trimmed.contains("=") {
                warnings.push(json!({
                    "line": line_num,
                    "message": "Input parameter 'Lots' has no default value",
                    "severity": "warning"
                }));
            }
        }
    }
    
    // Check for required sections
    let has_on_init = content.contains("int OnInit()");
    let has_on_tick = content.contains("void OnTick()");
    let has_on_deinit = content.contains("void OnDeinit");
    
    if !has_on_init {
        errors.push(json!({
            "line": 0,
            "message": "Missing OnInit() function - required for EA",
            "severity": "error"
        }));
    }
    
    if !has_on_tick && !content.contains("void OnTimer()") {
        warnings.push(json!({
            "line": 0,
            "message": "No OnTick() or OnTimer() found - EA won't respond to events",
            "severity": "warning"
        }));
    }
    
    let valid = errors.is_empty();
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": valid,
            "valid": valid,
            "path": path,
            "checks": {
                "has_on_init": has_on_init,
                "has_on_tick": has_on_tick,
                "has_on_deinit": has_on_deinit,
                "lines": lines.len()
            },
            "errors": if errors.is_empty() { None } else { Some(errors) },
            "warnings": if warnings.is_empty() { None } else { Some(warnings) },
            "hint": if valid { "Syntax looks good. Run compile_ea for full compilation check." } else { "Fix errors before compiling." }
        }).to_string() }],
        "isError": !valid
    }))
}

/// Check MT5 terminal status
pub async fn handle_check_mt5_status(config: &Config) -> Result<Value> {
    let mt5_dir = config.mt5_dir();
    let wine_exe = config.wine_executable.as_ref();
    
    // Check if MT5 files exist
    let terminal_exists = mt5_dir.as_ref().map(|d| d.join("terminal64.exe").exists()).unwrap_or(false);
    let metaeditor_exists = mt5_dir.as_ref().map(|d| d.join("metaeditor64.exe").exists()).unwrap_or(false);
    let tester_exists = mt5_dir.as_ref().map(|d| d.join("metatester64.exe").exists()).unwrap_or(false);
    
    // Check Wine
    let wine_ok = wine_exe.map(|w| Path::new(w).exists()).unwrap_or(false);
    
    // Try to get MT5 version (would need to actually run it, skip for now)
    let mut mt5_version = None;
    if wine_ok && terminal_exists {
        // Could run: wine terminal64.exe /version but it's complex
        mt5_version = Some("detected".to_string());
    }
    
    let all_ok = terminal_exists && metaeditor_exists && tester_exists && wine_ok;
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": all_ok,
            "terminal_ready": all_ok,
            "checks": {
                "mt5_dir_exists": mt5_dir.as_ref().map(|d| d.exists()).unwrap_or(false),
                "terminal64_exe": terminal_exists,
                "metaeditor64_exe": metaeditor_exists,
                "metatester64_exe": tester_exists,
                "wine_executable": wine_ok,
                "wine_path": wine_exe
            },
            "mt5_version": mt5_version,
            "current_account": config.current_account().map(|a| json!({
                "login": a.login,
                "server": a.server
            })),
            "hint": if all_ok { 
                "MT5 is ready for backtesting and optimization." 
            } else { 
                "Some components missing. Run verify_setup for detailed diagnostics." 
            }
        }).to_string() }],
        "isError": false
    }))
}

/// Create .set file template from EA
pub async fn handle_create_set_template(config: &Config, args: &Value) -> Result<Value> {
    let ea = args.get("ea")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("ea is required"))?;
    
    let output_path = args.get("output_path").and_then(|v| v.as_str());
    
    // Find EA source file
    let ea_path = if Path::new(ea).exists() {
        Path::new(ea).to_path_buf()
    } else if let Some(experts_dir) = &config.experts_dir {
        Path::new(experts_dir).join(format!("{}.mq5", ea))
    } else {
        return Err(anyhow::anyhow!("Cannot find EA: {}", ea));
    };
    
    if !ea_path.exists() {
        return Err(anyhow::anyhow!("EA file not found: {}", ea_path.display()));
    }
    
    let content = fs::read_to_string(&ea_path)?;
    let mut inputs = Vec::new();
    
    // Parse input declarations
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("input ") {
            // Parse: input type name = default; // comment
            let without_input = &trimmed[6..]; // Remove "input "
            
            // Extract comment if any
            let parts: Vec<&str> = without_input.split("//").collect();
            let decl = parts[0].trim();
            let comment = parts.get(1).map(|c| c.trim());
            
            // Parse type and name
            let tokens: Vec<&str> = decl.split_whitespace().collect();
            if tokens.len() >= 2 {
                let type_name = tokens[0];
                let rest = tokens[1..].to_vec().join(" ");
                
                // Parse name = value
                let name_value: Vec<&str> = rest.split('=').collect();
                let name = name_value[0].trim();
                let default_val = name_value.get(1).map(|v| v.trim().trim_end_matches(';')).unwrap_or("0");
                
                inputs.push(json!({
                    "name": name,
                    "type": type_name,
                    "default": default_val,
                    "description": comment
                }));
            }
        }
    }
    
    // Generate .set content
    let mut set_content = format!("; {} parameters generated by MT5-Quant\n", ea);
    set_content.push_str("; Format: name=value\n\n");
    
    for input in &inputs {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let default = input.get("default").and_then(|v| v.as_str()).unwrap_or("0");
        let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
        
        if !desc.is_empty() {
            set_content.push_str(&format!("; {}\n", desc));
        }
        set_content.push_str(&format!("{}={}\n\n", name, default));
    }
    
    // Determine output path
    let set_path = if let Some(path) = output_path {
        // Restrict writes to the tester profiles dir (or the EA's own dir as fallback).
        let allowed_base = if let Some(profiles_dir) = &config.tester_profiles_dir {
            Path::new(profiles_dir).to_path_buf()
        } else {
            ea_path.parent().unwrap_or(Path::new(".")).to_path_buf()
        };
        safe_output_path(path, &allowed_base)?
    } else if let Some(profiles_dir) = &config.tester_profiles_dir {
        Path::new(profiles_dir).join(format!("{}.set", ea))
    } else {
        ea_path.with_extension("set")
    };
    
    fs::write(&set_path, set_content)?;
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "ea": ea,
            "inputs_found": inputs.len(),
            "inputs": inputs,
            "set_file": set_path.to_string_lossy().to_string(),
            "hint": "Edit .set file to modify parameter values, then use with run_backtest set_file=..."
        }).to_string() }],
        "isError": false
    }))
}

/// Export backtest report to various formats
pub async fn handle_export_report(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;
    
    let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("csv");
    let output_path = args.get("output_path").and_then(|v| v.as_str());
    
    let path = Path::new(report_dir);
    let metrics_path = path.join("metrics.json");
    
    // Read metrics
    let metrics: Value = if metrics_path.exists() {
        fs::read_to_string(&metrics_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(json!({}))
    } else {
        json!({})
    };
    
    // Determine output file; restrict user-supplied path to the report directory.
    let output = match output_path {
        Some(p) => safe_output_path(p, path)?,
        None => path.join(format!("report.{}", format)),
    };
    
    let content = match format {
        "csv" => {
            // Simple CSV export of metrics
            let mut csv = "Metric,Value\n".to_string();
            if let Some(obj) = metrics.as_object() {
                for (key, value) in obj {
                    csv.push_str(&format!("{},{}\n", key, value));
                }
            }
            csv
        }
        "md" => {
            // Markdown format
            let mut md = format!("# Backtest Report: {}\n\n", metrics.get("expert").and_then(|v| v.as_str()).unwrap_or("Unknown"));
            md.push_str("## Summary\n\n");
            if let Some(obj) = metrics.as_object() {
                for (key, value) in obj {
                    md.push_str(&format!("- **{}**: {}\n", key, value));
                }
            }
            md
        }
        _ => metrics.to_string() // JSON fallback
    };
    
    fs::write(&output, content)?;
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "format": format,
            "output_file": output.to_string_lossy().to_string(),
            "source": report_dir,
            "hint": "Exported report ready for analysis or sharing."
        }).to_string() }],
        "isError": false
    }))
}

// === Wine & MT5 Debugging Tools ===

/// Diagnose Wine installation and prefix health
pub async fn handle_diagnose_wine(config: &Config, _args: &Value) -> Result<Value> {
    let mut diagnostics = json!({
        "wine_executable": null,
        "wine_version": null,
        "wine_prefix": null,
        "prefix_health": null,
        "prefix_exists": false,
        "prefix_size_mb": 0,
        "errors": Vec::<String>::new(),
        "warnings": Vec::<String>::new(),
    });
    
    // Check wine executable
    if let Some(wine_exe) = config.wine_executable.as_ref() {
        diagnostics["wine_executable"] = json!(wine_exe);
        
        // Get Wine version
        let version_output = std::process::Command::new(wine_exe)
            .arg("--version")
            .output();
        
        match version_output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                diagnostics["wine_version"] = json!(version);
            }
            _ => {
                diagnostics["errors"].as_array_mut().unwrap().push(
                    json!("Failed to get Wine version - Wine may not be properly installed")
                );
            }
        }
    } else {
        diagnostics["errors"].as_array_mut().unwrap().push(
            json!("Wine executable not configured")
        );
    }
    
    // Check Wine prefix
    if let Some(mt5_dir) = config.mt5_dir() {
        let wine_prefix = mt5_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent());
        
        if let Some(prefix) = wine_prefix {
            diagnostics["wine_prefix"] = json!(prefix.to_string_lossy().to_string());
            
            // Check if prefix exists
            let prefix_exists = prefix.exists();
            diagnostics["prefix_exists"] = json!(prefix_exists);
            
            if prefix_exists {
                // Calculate prefix size
                let mut total_size = 0u64;
                fn calculate_size(dir: &Path, total: &mut u64) {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() {
                                if let Ok(meta) = entry.metadata() {
                                    *total += meta.len();
                                }
                            } else if path.is_dir() {
                                calculate_size(&path, total);
                            }
                        }
                    }
                }
                calculate_size(prefix, &mut total_size);
                diagnostics["prefix_size_mb"] = json!((total_size / 1024 / 1024) as i64);
                
                // Check critical directories
                let system32 = prefix.join("drive_c/windows/system32");
                let program_files = prefix.join("drive_c/Program Files");
                
                if !system32.exists() {
                    diagnostics["errors"].as_array_mut().unwrap().push(
                        json!("Wine prefix missing system32 directory - prefix may be corrupted")
                    );
                    diagnostics["prefix_health"] = json!("corrupted");
                } else if !program_files.exists() {
                    diagnostics["warnings"].as_array_mut().unwrap().push(
                        json!("Program Files directory not found")
                    );
                    diagnostics["prefix_health"] = json!("incomplete");
                } else {
                    diagnostics["prefix_health"] = json!("healthy");
                }
                
                // Check for recent Wine errors
                let wine_log = prefix.join("wine.log");
                if wine_log.exists() {
                    if let Ok(content) = fs::read_to_string(&wine_log) {
                        let recent_errors: Vec<&str> = content.lines()
                            .filter(|l| l.contains("err:") || l.contains("fixme:"))
                            .rev()
                            .take(10)
                            .collect();
                        if !recent_errors.is_empty() {
                            diagnostics["recent_wine_errors"] = json!(recent_errors);
                        }
                    }
                }
            } else {
                diagnostics["errors"].as_array_mut().unwrap().push(
                    json!("Wine prefix directory does not exist")
                );
                diagnostics["prefix_health"] = json!("missing");
            }
        } else {
            diagnostics["errors"].as_array_mut().unwrap().push(
                json!("Could not determine Wine prefix from MT5 directory")
            );
        }
    } else {
        diagnostics["errors"].as_array_mut().unwrap().push(
            json!("MT5 directory not configured")
        );
    }
    
    let has_errors = !diagnostics["errors"].as_array().unwrap().is_empty();
    
    Ok(json!({
        "content": [{ "type": "text", "text": diagnostics.to_string() }],
        "isError": has_errors
    }))
}

/// Get MT5 terminal logs
pub async fn handle_get_mt5_logs(config: &Config, args: &Value) -> Result<Value> {
    let log_type = args.get("log_type").and_then(|v| v.as_str()).unwrap_or("terminal");
    let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let search = args.get("search").and_then(|v| v.as_str());
    
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 directory not configured"))?;
    
    let log_path = match log_type {
        "terminal" => mt5_dir.join("logs").join(format!("{}", chrono::Local::now().format("%Y%m%d"))),
        "tester" => mt5_dir.join("Tester").join("logs"),
        "metaeditor" => mt5_dir.join("MetaEditor").join("logs"),
        _ => mt5_dir.join("logs"),
    };
    
    let mut result = json!({
        "log_type": log_type,
        "log_path": log_path.to_string_lossy().to_string(),
        "found": false,
        "lines_total": 0,
        "lines_returned": 0,
        "content": Vec::<String>::new(),
    });
    
    // Find log files
    let mut log_files: Vec<_> = Vec::new();
    if log_path.exists() {
        if let Ok(entries) = fs::read_dir(&log_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "log" {
                            if let Ok(meta) = entry.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    log_files.push((path, modified));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Sort by modification time (newest first)
    log_files.sort_by(|a, b| b.1.cmp(&a.1));
    
    if let Some((latest_log, _)) = log_files.first() {
        result["found"] = json!(true);
        
        if let Ok(content) = fs::read_to_string(latest_log) {
            let all_lines: Vec<&str> = content.lines().collect();
            result["lines_total"] = json!(all_lines.len());
            
            // Filter and limit lines
            let mut filtered: Vec<&str> = all_lines.clone();
            
            // Apply search filter
            if let Some(search_term) = search {
                let search_lower = search_term.to_lowercase();
                filtered.retain(|line| line.to_lowercase().contains(&search_lower));
            }
            
            // Get last N lines
            let start = filtered.len().saturating_sub(lines);
            let final_lines: Vec<String> = filtered[start..].iter().map(|s| s.to_string()).collect();
            
            result["lines_returned"] = json!(final_lines.len());
            result["content"] = json!(final_lines);
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}

/// Search MT5 logs for error patterns
pub async fn handle_search_mt5_errors(config: &Config, args: &Value) -> Result<Value> {
    let error_patterns = vec![
        "error", "failed", "crash", "exception", "access violation",
        "out of memory", "cannot", "unable to", "terminated"
    ];
    let hours_back = args.get("hours_back").and_then(|v| v.as_u64()).unwrap_or(24);
    let max_errors = args.get("max_errors").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 directory not configured"))?;
    
    let mut errors_found = Vec::new();
    let logs_dir = mt5_dir.join("logs");
    let cutoff_time = std::time::SystemTime::now() - std::time::Duration::from_secs(hours_back * 3600);
    
    // Search recent log files
    if logs_dir.exists() {
        if let Ok(entries) = fs::read_dir(&logs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "log").unwrap_or(false) {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if modified >= cutoff_time {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    for (i, line) in content.lines().enumerate() {
                                        let line_lower = line.to_lowercase();
                                        for pattern in &error_patterns {
                                            if line_lower.contains(pattern) {
                                                errors_found.push(json!({
                                                    "file": path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                                                    "line": i + 1,
                                                    "content": line.trim().to_string(),
                                                    "pattern": pattern,
                                                }));
                                                if errors_found.len() >= max_errors {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if errors_found.len() >= max_errors {
                    break;
                }
            }
        }
    }
    
    let result = json!({
        "hours_searched": hours_back,
        "errors_found": errors_found.len(),
        "max_errors": max_errors,
        "errors": errors_found,
        "suggestion": if errors_found.is_empty() {
            "No errors found in recent logs. Check get_mt5_logs for full log content."
        } else {
            "Found potential errors. Review the 'content' field for details."
        },
    });
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}

/// Check MT5 process status
pub async fn handle_check_mt5_process(_config: &Config, _args: &Value) -> Result<Value> {
    use std::process::Command;
    
    let mut result = json!({
        "is_running": false,
        "processes": Vec::<serde_json::Value>::new(),
        "wine_server_running": false,
        "total_instances": 0,
    });
    
    #[cfg(target_os = "macos")]
    {
        // Check for MT5 processes
        let ps_output = Command::new("ps")
            .args(["aux"])
            .output();
        
        if let Ok(output) = ps_output {
            let content = String::from_utf8_lossy(&output.stdout);
            let mut processes = Vec::new();
            let mut mt5_count = 0;
            let mut wine_server = false;
            
            for line in content.lines() {
                let line_lower = line.to_lowercase();
                
                if line_lower.contains("terminal64") || line_lower.contains("metatrader") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 11 {
                        processes.push(json!({
                            "pid": parts[1],
                            "cpu": parts[2],
                            "mem": parts[3],
                            "command": parts[10..].join(" "),
                        }));
                        mt5_count += 1;
                    }
                }
                
                if line_lower.contains("wineserver") {
                    wine_server = true;
                }
            }
            
            result["processes"] = json!(processes);
            result["is_running"] = json!(mt5_count > 0);
            result["total_instances"] = json!(mt5_count);
            result["wine_server_running"] = json!(wine_server);
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        let ps_output = Command::new("ps")
            .args(["aux"])
            .output();
        
        if let Ok(output) = ps_output {
            let content = String::from_utf8_lossy(&output.stdout);
            let mut processes = Vec::new();
            let mut mt5_count = 0;
            let mut wine_server = false;
            
            for line in content.lines() {
                let line_lower = line.to_lowercase();
                
                if line_lower.contains("terminal64") || line_lower.contains("metatrader") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 11 {
                        processes.push(json!({
                            "pid": parts[1],
                            "cpu": parts[2],
                            "mem": parts[3],
                            "command": parts[10..].join(" "),
                        }));
                        mt5_count += 1;
                    }
                }
                
                if line_lower.contains("wineserver") {
                    wine_server = true;
                }
            }
            
            result["processes"] = json!(processes);
            result["is_running"] = json!(mt5_count > 0);
            result["total_instances"] = json!(mt5_count);
            result["wine_server_running"] = json!(wine_server);
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}

/// Kill stuck MT5 process
pub async fn handle_kill_mt5_process(_config: &Config, args: &Value) -> Result<Value> {
    use std::process::Command;
    
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let pid = args.get("pid").and_then(|v| v.as_str());
    
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        // Get list of MT5 processes
        let ps_output = Command::new("ps")
            .args(["aux"])
            .output();
        
        if let Ok(output) = ps_output {
            let content = String::from_utf8_lossy(&output.stdout);
            
            for line in content.lines() {
                let line_lower = line.to_lowercase();
                
                let should_kill = if let Some(target_pid) = pid {
                    line.contains(target_pid) && (line_lower.contains("terminal64") || line_lower.contains("metatrader"))
                } else {
                    line_lower.contains("terminal64") || line_lower.contains("metatrader")
                };
                
                if should_kill {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let process_pid = parts[1];
                        let signal = if force { "-9" } else { "-15" };
                        
                        match Command::new("kill").args([signal, process_pid]).output() {
                            Ok(_) => killed.push(process_pid.to_string()),
                            Err(e) => failed.push(format!("{}: {}", process_pid, e)),
                        }
                    }
                }
            }
        }
        
        // Also kill wineserver if force=true
        if force {
            let _ = Command::new("killall").arg("wineserver").output();
        }
    }
    
    let message = if killed.is_empty() {
        "No MT5 processes found to kill".to_string()
    } else {
        format!("Killed {} MT5 process(es)", killed.len())
    };
    
    let result = json!({
        "killed": killed,
        "failed": failed,
        "force": force,
        "message": message,
    });
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": !failed.is_empty()
    }))
}

/// Check system resources for MT5
pub async fn handle_check_system_resources(_config: &Config, _args: &Value) -> Result<Value> {
    use std::process::Command;
    
    let mut result = json!({
        "disk_space": null,
        "memory": null,
        "cpu_cores": 0,
        "recommendations": Vec::<String>::new(),
    });
    
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        // Check disk space
        let df_output = Command::new("df")
            .args(["-h", "/"])
            .output();
        
        if let Ok(output) = df_output {
            let content = String::from_utf8_lossy(&output.stdout);
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    result["disk_space"] = json!({
                        "filesystem": parts[0],
                        "size": parts[1],
                        "used": parts[2],
                        "available": parts[3],
                        "use_percent": parts[4],
                    });
                    
                    // Check if low on space
                    let use_pct = parts[4].trim_end_matches('%').parse::<u32>().unwrap_or(0);
                    if use_pct > 90 {
                        result["recommendations"].as_array_mut().unwrap().push(
                            json!("Disk space critically low. Clean MT5 cache with clean_cache.")
                        );
                    } else if use_pct > 80 {
                        result["recommendations"].as_array_mut().unwrap().push(
                            json!("Disk space getting low. Consider cleaning cache.")
                        );
                    }
                }
            }
        }
        
        // Check memory
        #[cfg(target_os = "macos")]
        {
            let vm_output = Command::new("vm_stat").output();
            if let Ok(output) = vm_output {
                let content = String::from_utf8_lossy(&output.stdout);
                // Parse vm_stat output
                let mut free_pages = 0u64;
                let mut active_pages = 0u64;
                let mut inactive_pages = 0u64;
                
                for line in content.lines() {
                    if line.contains("Pages free:") {
                        free_pages = line.split_whitespace().nth(2).unwrap_or("0").trim_end_matches('.').parse().unwrap_or(0);
                    } else if line.contains("Pages active:") {
                        active_pages = line.split_whitespace().nth(2).unwrap_or("0").trim_end_matches('.').parse().unwrap_or(0);
                    } else if line.contains("Pages inactive:") {
                        inactive_pages = line.split_whitespace().nth(2).unwrap_or("0").trim_end_matches('.').parse().unwrap_or(0);
                    }
                }
                
                let page_size = 4096u64;
                let total_mb = ((free_pages + active_pages + inactive_pages) * page_size) / 1024 / 1024;
                let free_mb = (free_pages * page_size) / 1024 / 1024;
                
                result["memory"] = json!({
                    "total_mb": total_mb,
                    "free_mb": free_mb,
                    "unit": "MB",
                });
                
                if free_mb < 2048 {
                    result["recommendations"].as_array_mut().unwrap().push(
                        json!("Low memory available. MT5 may crash during large optimizations.")
                    );
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            let mem_output = Command::new("free").args(["-m"]).output();
            if let Ok(output) = mem_output {
                let content = String::from_utf8_lossy(&output.stdout);
                for line in content.lines() {
                    if line.starts_with("Mem:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 4 {
                            result["memory"] = json!({
                                "total_mb": parts[1].parse::<u64>().unwrap_or(0),
                                "used_mb": parts[2].parse::<u64>().unwrap_or(0),
                                "free_mb": parts[3].parse::<u64>().unwrap_or(0),
                                "unit": "MB",
                            });
                        }
                    }
                }
            }
        }
        
        // Get CPU cores
        let nproc_output = Command::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output();
        
        if let Ok(output) = nproc_output {
            let cores = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0);
            result["cpu_cores"] = json!(cores);
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}

/// Validate MT5 configuration files
pub async fn handle_validate_mt5_config(config: &Config, _args: &Value) -> Result<Value> {
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 directory not configured"))?;
    
    let mut result = json!({
        "terminal_ini": null,
        "tester_ini": null,
        "config_files_found": Vec::<String>::new(),
        "errors": Vec::<String>::new(),
        "warnings": Vec::<String>::new(),
    });
    
    // Check terminal.ini
    let terminal_ini = mt5_dir.join("terminal.ini");
    if terminal_ini.exists() {
        result["config_files_found"].as_array_mut().unwrap().push(json!("terminal.ini"));
        
        if let Ok(content) = fs::read_to_string(&terminal_ini) {
            // Check for common issues
            if !content.contains("[Common]") {
                result["errors"].as_array_mut().unwrap().push(
                    json!("terminal.ini missing [Common] section")
                );
            }
            
            // Extract key settings
            let mut settings = serde_json::Map::new();
            for line in content.lines() {
                if line.starts_with("Login=") {
                    settings.insert("login".to_string(), json!(line.trim_start_matches("Login=")));
                } else if line.starts_with("Server=") {
                    settings.insert("server".to_string(), json!(line.trim_start_matches("Server=")));
                } else if line.starts_with("Expert=") {
                    settings.insert("expert".to_string(), json!(line.trim_start_matches("Expert=")));
                }
            }
            result["terminal_ini"] = json!(settings);
        } else {
            result["errors"].as_array_mut().unwrap().push(
                json!("Could not read terminal.ini")
            );
        }
    } else {
        result["warnings"].as_array_mut().unwrap().push(
            json!("terminal.ini not found")
        );
    }
    
    // Check for tester config
    let tester_dir = mt5_dir.join("Tester");
    if tester_dir.exists() {
        if let Ok(entries) = fs::read_dir(&tester_dir) {
            let ini_files: Vec<String> = entries
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension()?.to_str()? == "ini" {
                        Some(p.file_name()?.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect();
            
            if !ini_files.is_empty() {
                result["tester_ini"] = json!(ini_files);
            }
        }
    }
    
    // Check for common problems
    let experts_dir = mt5_dir.join("MQL5").join("Experts");
    if !experts_dir.exists() {
        result["errors"].as_array_mut().unwrap().push(
            json!("MQL5/Experts directory not found - MT5 installation may be incomplete")
        );
    }
    
    let has_errors = !result["errors"].as_array().unwrap().is_empty();
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": has_errors
    }))
}

/// Get Wine prefix detailed information
pub async fn handle_get_wine_prefix_info(config: &Config, _args: &Value) -> Result<Value> {
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 directory not configured"))?;
    
    let wine_prefix = mt5_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent());
    
    let mut result = json!({
        "prefix_path": null,
        "exists": false,
        "windows_version": null,
        "dll_overrides": Vec::<String>::new(),
        "installed_programs": Vec::<String>::new(),
        "registry_files": Vec::<String>::new(),
        "drive_c_size_mb": 0,
    });
    
    if let Some(prefix) = wine_prefix {
        result["prefix_path"] = json!(prefix.to_string_lossy().to_string());
        result["exists"] = json!(prefix.exists());
        
        if prefix.exists() {
            // Check Windows version
            let system_reg = prefix.join("system.reg");
            if system_reg.exists() {
                if let Ok(content) = fs::read_to_string(&system_reg) {
                    for line in content.lines().take(50) {
                        if line.contains("\"ProductName\"") {
                            let parts: Vec<&str> = line.split('"').collect();
                            if parts.len() >= 4 {
                                result["windows_version"] = json!(parts[3]);
                            }
                        }
                    }
                }
            }
            
            // Calculate drive_c size
            let drive_c = prefix.join("drive_c");
            if drive_c.exists() {
                let mut size = 0u64;
                fn calc_size(dir: &Path, total: &mut u64) {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for e in entries.flatten() {
                            let p = e.path();
                            if p.is_file() {
                                if let Ok(m) = e.metadata() {
                                    *total += m.len();
                                }
                            } else if p.is_dir() {
                                calc_size(&p, total);
                            }
                        }
                    }
                }
                calc_size(&drive_c, &mut size);
                result["drive_c_size_mb"] = json!((size / 1024 / 1024) as i64);
            }
            
            // Check for installed programs
            let prog_files = prefix.join("drive_c").join("Program Files");
            if prog_files.exists() {
                if let Ok(entries) = fs::read_dir(&prog_files) {
                    let programs: Vec<String> = entries
                        .flatten()
                        .filter_map(|e| {
                            let p = e.path();
                            if p.is_dir() {
                                Some(p.file_name()?.to_string_lossy().to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                    result["installed_programs"] = json!(programs);
                }
            }
            
            // List registry files
            let reg_files: Vec<String> = vec![
                "system.reg", "user.reg", "userdef.reg"
            ]
            .into_iter()
            .filter(|f| prefix.join(f).exists())
            .map(|f| f.to_string())
            .collect();
            result["registry_files"] = json!(reg_files);
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}

/// Get backtest crash/failure information
pub async fn handle_get_backtest_crash_info(config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir").and_then(|v| v.as_str());
    let check_recent = args.get("check_recent").and_then(|v| v.as_bool()).unwrap_or(true);
    let hours_back = args.get("hours_back").and_then(|v| v.as_u64()).unwrap_or(6);
    
    let mut result = json!({
        "crashes_found": Vec::<serde_json::Value>::new(),
        "recent_failures": 0,
        "common_patterns": Vec::<String>::new(),
    });
    
    // Check specific report directory if provided
    if let Some(dir) = report_dir {
        let path = Path::new(dir);
        if path.exists() {
            // Check for incomplete markers
            let incomplete_marker = path.join(".incomplete");
            let error_log = path.join("error.log");
            
            if incomplete_marker.exists() {
                result["crashes_found"].as_array_mut().unwrap().push(json!({
                    "report_dir": dir,
                    "type": "incomplete",
                    "reason": "Backtest was interrupted or timed out",
                }));
            }
            
            if error_log.exists() {
                if let Ok(content) = fs::read_to_string(&error_log) {
                    result["crashes_found"].as_array_mut().unwrap().push(json!({
                        "report_dir": dir,
                        "type": "error_log",
                        "content": content.lines().take(20).collect::<Vec<_>>().join("\n"),
                    }));
                }
            }
            
            // Check deal count in DB
            let db = ReportDb::new(&Config::db_path());
            if db.init().is_ok() {
                match db.get_by_report_dir(dir) {
                    Ok(Some(entry)) => {
                        let deal_count = db.get_deals(&entry.id)
                            .map(|d| d.len())
                            .unwrap_or(0);
                        if deal_count == 0 {
                            result["crashes_found"].as_array_mut().unwrap().push(json!({
                                "report_dir": dir,
                                "type": "empty_deals",
                                "reason": "No deals stored in DB - EA did not trade or extraction failed",
                            }));
                        }
                    }
                    Ok(None) | Err(_) => {
                        // Not in DB at all means extraction never completed
                        let metrics_exists = path.join("metrics.json").exists();
                        if !metrics_exists {
                            result["crashes_found"].as_array_mut().unwrap().push(json!({
                                "report_dir": dir,
                                "type": "missing_deals",
                                "reason": "Report not found in DB and no metrics.json - backtest likely crashed before extraction",
                            }));
                        }
                    }
                }
            }
        }
    }
    
    // Check recent reports if requested
    if check_recent {
        let reports_dir_str = config.get("reports_dir");
        let reports_dir = Path::new(&reports_dir_str);
        if reports_dir.exists() {
            let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(hours_back * 3600);
            
            if let Ok(entries) = fs::read_dir(&reports_dir) {
                let mut failures = 0;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Ok(meta) = entry.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if modified >= cutoff {
                                    // Check for failure indicators
                                    let has_metrics = path.join("metrics.json").exists();
                                    let has_incomplete = path.join(".incomplete").exists();

                                    if !has_metrics || has_incomplete {
                                        failures += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                result["recent_failures"] = json!(failures);
            }
        }
    }
    
    // Analyze common patterns
    let crashes = result["crashes_found"].as_array().unwrap();
    if !crashes.is_empty() {
        let types: Vec<String> = crashes.iter()
            .filter_map(|c| c.get("type").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        
        if types.contains(&"missing_deals".to_string()) {
            result["common_patterns"].as_array_mut().unwrap().push(
                json!("Report not in DB and no metrics.json suggests MT5 crashed before extraction completed")
            );
        }
        if types.contains(&"incomplete".to_string()) {
            result["common_patterns"].as_array_mut().unwrap().push(
                json!("Incomplete markers indicate interruptions - check system resources")
            );
        }
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}
