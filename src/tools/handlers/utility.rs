use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::models::Config;
use walkdir::WalkDir;

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
pub async fn handle_compare_backtests(config: &Config, args: &Value) -> Result<Value> {
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
        
        for (i, comp) in comparisons.iter().enumerate().skip(1) {
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
        Path::new(path).to_path_buf()
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
    let deals_path = path.join("deals.csv");
    
    // Read metrics
    let metrics: Value = if metrics_path.exists() {
        fs::read_to_string(&metrics_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(json!({}))
    } else {
        json!({})
    };
    
    // Determine output file
    let output = match output_path {
        Some(p) => Path::new(p).to_path_buf(),
        None => path.join(format!("report.{}"
, format))
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
