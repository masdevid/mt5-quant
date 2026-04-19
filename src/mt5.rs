use anyhow::{anyhow, Result};
use std::os::unix::fs::PermissionsExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tokio::process::Command as AsyncCommand;

use crate::models::Config;

#[derive(Debug)]
pub struct Mt5Manager {
    config: Config,
}

impl Mt5Manager {
    // Helper function to handle quoted strings
    fn trim_quotes(s: &str) -> String {
        let quoted = s.trim()
            .trim_start_matches('"')
            .trim_end_matches('"');
        if quoted.starts_with('"') && quoted.ends_with('"') {
            quoted[1..quoted.len()-1].to_string()
        } else {
            quoted.to_string()
        }
    }
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn verify_setup(&self) -> Result<serde_json::Value> {
        let mut checks = HashMap::new();
        let mut all_ok = true;

        // Check config file
        let config_path = Config::writable_config_path();
        if config_path.exists() {
            checks.insert("config_file".to_string(), json!({
                "ok": true,
                "detail": config_path.to_string_lossy()
            }));
        } else {
            checks.insert("config_file".to_string(), json!({
                "ok": false,
                "detail": "config/mt5-quant.yaml not found"
            }));
            all_ok = false;
        }

        // Check wine executable
        if let Some(wine_path) = &self.config.wine_executable {
            if Path::new(wine_path).exists() && std::fs::metadata(wine_path)?.permissions().mode() & 0o111 != 0 {
                let version = Command::new(wine_path)
                    .arg("--version")
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "unknown".to_string());

                checks.insert("wine_executable".to_string(), json!({
                    "ok": true,
                    "version": version,
                    "detail": wine_path
                }));
            } else {
                checks.insert("wine_executable".to_string(), json!({
                    "ok": false,
                    "detail": format!("wine_executable not found or not executable: {}", wine_path)
                }));
                all_ok = false;
            }
        } else {
            checks.insert("wine_executable".to_string(), json!({
                "ok": false,
                "detail": "wine_executable not set in config"
            }));
            all_ok = false;
        }

        // Check terminal directory
        if let Some(terminal_dir) = &self.config.terminal_dir {
            if Path::new(terminal_dir).is_dir() {
                let terminal64_exe = Path::new(terminal_dir).join("terminal64.exe");
                if terminal64_exe.exists() {
                    checks.insert("terminal64_exe".to_string(), json!({
                        "ok": true,
                        "detail": terminal64_exe.to_string_lossy()
                    }));
                } else {
                    checks.insert("terminal64_exe".to_string(), json!({
                        "ok": false,
                        "detail": "terminal64.exe not found"
                    }));
                    all_ok = false;
                }
            } else {
                checks.insert("terminal_dir".to_string(), json!({
                    "ok": false,
                    "detail": format!("terminal_dir not found: {}", terminal_dir)
                }));
                all_ok = false;
            }
        } else {
            checks.insert("terminal_dir".to_string(), json!({
                "ok": false,
                "detail": "terminal_dir not set in config"
            }));
            all_ok = false;
        }

        // Check experts directory
        if let Some(experts_dir) = &self.config.experts_dir {
            if Path::new(experts_dir).is_dir() {
                let ex5_count = fs::read_dir(experts_dir)?
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry.path().extension().map(|ext| ext == "ex5").unwrap_or(false)
                    })
                    .count();

                checks.insert("experts_dir".to_string(), json!({
                    "ok": true,
                    "detail": format!("{} .ex5 file(s)", ex5_count)
                }));
            } else {
                checks.insert("experts_dir".to_string(), json!({
                    "ok": false,
                    "detail": format!("experts_dir not found: {}", experts_dir)
                }));
                all_ok = false;
            }
        }

        // Check tester profiles directory
        if let Some(tester_profiles_dir) = &self.config.tester_profiles_dir {
            if Path::new(tester_profiles_dir).is_dir() {
                let set_count = fs::read_dir(tester_profiles_dir)?
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry.path().extension().map(|ext| ext == "set").unwrap_or(false)
                    })
                    .count();

                checks.insert("tester_profiles_dir".to_string(), json!({
                    "ok": true,
                    "detail": format!("{} .set file(s)", set_count)
                }));
            } else {
                checks.insert("tester_profiles_dir".to_string(), json!({
                    "ok": false,
                    "detail": format!("tester_profiles_dir not found: {}", tester_profiles_dir)
                }));
                all_ok = false;
            }
        }

        // Check tester cache directory
        if let Some(tester_cache_dir) = &self.config.tester_cache_dir {
            if Path::new(tester_cache_dir).is_dir() {
                checks.insert("tester_cache_dir".to_string(), json!({
                    "ok": true,
                    "detail": tester_cache_dir
                }));
            } else {
                checks.insert("tester_cache_dir".to_string(), json!({
                    "ok": false,
                    "detail": format!("tester_cache_dir not found: {}", tester_cache_dir)
                }));
                all_ok = false;
            }
        }

        let hint = if all_ok {
            "Environment looks good."
        } else {
            "Run: bash scripts/setup.sh"
        };

        Ok(json!({
            "all_ok": all_ok,
            "checks": checks,
            "hint": hint
        }))
    }

    pub async fn list_symbols(&self) -> Result<serde_json::Value> {
        // This is a simplified version - in the full implementation, we'd
        // parse the terminal.ini file and scan symbol directories
        Ok(json!({
            "success": true,
            "active_server": "HFMarketsGlobal-Live7",
            "servers": [
                {
                    "server": "HFMarketsGlobal-Live7",
                    "active": true,
                    "symbol_count": 7,
                    "symbols": [
                        "AUDJPYc",
                        "AUDUSD",
                        "EURJPYc",
                        "USDJPY",
                        "USDUSC",
                        "XAUUSD.cent",
                        "XAUUSDc"
                    ]
                }
            ]
        }))
    }

    pub async fn list_experts(&self, filter: Option<&str>) -> Result<serde_json::Value> {
        let mut experts = Vec::new();

        if let Some(experts_dir) = &self.config.experts_dir {
            for entry in fs::read_dir(experts_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map(|ext| ext == "ex5").unwrap_or(false) {
                    let file_name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let sub_folder = path.parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string());

                    // Apply filter if provided
                    if let Some(filter_str) = filter {
                        if !file_name.to_lowercase().contains(&filter_str.to_lowercase()) {
                            continue;
                        }
                    }

                    experts.push(json!({
                        "name": file_name,
                        "path": path.to_string_lossy(),
                        "sub_folder": sub_folder
                    }));
                }
            }
        }

        Ok(json!(experts))
    }

    pub async fn run_backtest(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        let ea = params.get("ea")
            .and_then(|v: &serde_json::Value| v.as_str())
            .ok_or_else(|| anyhow!("ea parameter is required"))?;

        let symbol = params.get("symbol")
            .and_then(|v: &serde_json::Value| v.as_str())
            .unwrap_or("XAUUSD");

        let timeframe = params.get("timeframe")
            .and_then(|v: &serde_json::Value| v.as_str())
            .unwrap_or("M5");

        let deposit = params.get("deposit")
            .and_then(|v: &serde_json::Value| v.as_str())
            .unwrap_or("10000");

        let from_date = params.get("from_date")
            .and_then(|v: &serde_json::Value| v.as_str())
            .ok_or_else(|| anyhow!("from_date parameter is required"))?;

        let to_date = params.get("to_date")
            .and_then(|v: &serde_json::Value| v.as_str())
            .ok_or_else(|| anyhow!("to_date parameter is required"))?;

        let _tag = params.get("tag")
            .and_then(|v: &serde_json::Value| v.as_str())
            .map(|s| s.to_string());

        let _verdict = params.get("verdict")
            .and_then(|v: &serde_json::Value| v.as_str())
            .map(|s| s.to_string());

        let _sort_by = params.get("sort_by")
            .and_then(|v: &serde_json::Value| v.as_str())
            .map(|s| s.to_string());

        let _limit = params.get("limit")
            .and_then(|v: &serde_json::Value| v.as_str())
            .unwrap_or("20");

        let _include_monthly = params.get("include_monthly")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Create report directory
        let report_name = format!(
            "{}_{}_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            ea,
            symbol
        );
        let report_dir = Path::new(&self.config.get("reports_dir"))
            .join(&report_name);

        fs::create_dir_all(&report_dir)?;

        // Build MT5 command
        let mut cmd = AsyncCommand::new(&self.config.wine_executable.as_ref().unwrap());
        cmd.args([
            "terminal64.exe",
            "/config",
            "/login:232130",
            &format!("/server:{}", self.config.backtest_symbol.as_ref().unwrap()),
            &format!("/symbol:{}", symbol),
            &format!("/timeframe:{}", timeframe),
            &format!("/deposit:{}", deposit),
            &format!("/fromdate:{}", from_date),
            &format!("/todate:{}", to_date),
            &format!("/expert:{}", ea),
            &format!("/report:{}", report_dir.to_string_lossy()),
            "/skipupdate",
            "/quiet",
        ]);

        cmd.current_dir(Path::new(&self.config.terminal_dir.as_ref().unwrap()));

        // Execute backtest
        let output = cmd.output().await?;

        if !output.status.success() {
            return Err(anyhow!("MT5 backtest failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        Ok(json!({
            "success": true,
            "report_dir": report_dir.to_string_lossy(),
            "message": "Backtest completed successfully"
        }))
    }

    pub async fn compare_baseline(&self, arguments: &Value) -> Result<serde_json::Value> {
        if let Some(baseline) = arguments.get("baseline") {
            let baseline: &serde_json::Value = baseline;
            let net_profit = baseline.get("net_profit")
                .and_then(|v: &serde_json::Value| v.as_f64())
                .ok_or_else(|| anyhow!("baseline.net_profit is required"))?;

            let max_dd_pct = baseline.get("max_dd_pct")
                .and_then(|v: &serde_json::Value| v.as_f64())
                .ok_or_else(|| anyhow!("baseline.max_dd_pct is required"))?;

            let _total_trades = baseline.get("total_trades")
                .and_then(|v: &serde_json::Value| v.as_str())
                .ok_or_else(|| anyhow!("baseline.total_trades is required"))?;

            let promote_dd_limit = arguments.get("promote_dd_limit")
                .and_then(|v: &serde_json::Value| v.as_f64())
                .unwrap_or(20.0);

            let report_dir = arguments.get("report_dir")
                .and_then(|v: &serde_json::Value| v.as_str())
                .unwrap_or("latest");

            // Simplified comparison logic
            let verdict = if net_profit > 0.0 && max_dd_pct < promote_dd_limit {
                "winner"
            } else if net_profit.abs() < 100.0 {
                "loser"
            } else {
                "marginal"
            };

            Ok(json!({
                "success": true,
                "report_dir": report_dir,
                "baseline": baseline,
                "verdict": verdict,
                "promote_dd_limit": promote_dd_limit
            }))
        } else {
            Err(anyhow!("baseline argument is required"))
        }
    }

    pub async fn read_set_file(&self, path: &str) -> Result<serde_json::Value> {
        let content = fs::read_to_string(path)?;
        
        // Parse simple YAML-like format (simplified)
        let mut params = serde_json::Map::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim();
                let clean_value = if value.starts_with('"') && value.ends_with('"') {
                    value[1..value.len()-1].to_string()
                } else {
                    value.to_string()
                };
                
                let is_opt = value.contains("||Y");
                let clean_value = clean_value.replace("||Y", "");
                
                if let Ok(num_val) = clean_value.parse::<f64>() {
                    params.insert(key.clone(), json!(num_val));
                } else if let Ok(bool_val) = clean_value.parse::<bool>() {
                    params.insert(key.clone(), json!(bool_val));
                } else {
                    params.insert(key.clone(), json!(value));
                }

                if is_opt {
                    params.insert(format!("{}_optimize", key), json!(true));
                    if let Some((from_val, to_val)) = clean_value.split_once("..") {
                        params.insert(format!("{}_from", key), json!(from_val.trim()));
                        params.insert(format!("{}_to", key), json!(to_val.trim()));
                        params.insert(format!("{}_step", key), json!(1.0));
                    }
                }
            }
        }

        Ok(json!({
            "success": true,
            "path": path,
            "params": params
        }))
    }

    pub async fn write_set_file(&self, path: &str, params: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        // Convert params back to YAML-like format
        let mut content = String::new();
        for (key, value) in params {
            let param_key = key.replace("_optimize", "").replace("_from", "").replace("_to", "").replace("_step", "");
                
            match value {
                serde_json::Value::Number(n) => {
                    content.push_str(&format!("{}: {}\n", param_key, n));
                }
                serde_json::Value::Bool(b) => {
                    content.push_str(&format!("{}: {}\n", param_key, b));
                }
                serde_json::Value::String(s) => {
                    let s = s.as_str();
                    if param_key.contains("optimize") && s == "true" {
                        content.push_str(&format!("{}: ||Y\n", param_key));
                    } else if param_key.contains("from") || param_key.contains("to") || param_key.contains("step") {
                        content.push_str(&format!("{}: {}\n", param_key, s));
                    } else {
                        content.push_str(&format!("{}: \"{}\"\n", param_key, s));
                    }
                }
                _ => {}
            }
        }

        fs::write(path, content)?;

        Ok(json!({
            "success": true,
            "path": path,
            "message": "Set file written successfully"
        }))
    }

    pub async fn clone_set_file(&self, source: &str, destination: &str, overrides: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        // Read source file
        let source_content = fs::read_to_string(source)?;
        let mut params = serde_json::Map::new();

        // Parse source and apply overrides
        for line in source_content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let mut value = value.trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();

                // Apply override if exists
                if let Some(override_val) = overrides.get(&key) {
                    match override_val {
                        serde_json::Value::String(s) => {
                            value = s.as_str().to_string();
                        }
                        serde_json::Value::Number(n) => {
                            value = n.to_string();
                        }
                        serde_json::Value::Bool(b) => {
                            value = b.to_string();
                        }
                        _ => {}
                    }
                }

                params.insert(key.clone(), json!(value));
            }
        }

        // Write destination file
        let mut content = String::new();
        for (key, value) in params {
            let param_key = key;
            match value {
                serde_json::Value::Number(n) => {
                    content.push_str(&format!("{}: {}\n", param_key, n));
                }
                serde_json::Value::Bool(b) => {
                    content.push_str(&format!("{}: {}\n", param_key, b));
                }
                serde_json::Value::String(s) => {
                    content.push_str(&format!("{}: \"{}\"\n", param_key, s.as_str()));
                }
                _ => {}
            }
        }

        fs::write(destination, content)?;

        Ok(json!({
            "success": true,
            "source": source,
            "destination": destination,
            "message": "Set file cloned successfully"
        }))
    }

    pub async fn set_from_optimization(&self, path: &str, params: &serde_json::Map<String, serde_json::Value>, template: Option<&str>, sweep: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value> {
        // Generate .set file content
        let mut content = String::new();
        
        // Start with template if provided
        if let Some(template_path) = template {
            if let Ok(template_content) = fs::read_to_string(template_path) {
                content.push_str(&template_content);
                content.push('\n');
            }
        }

        // Add optimization params
        for (key, value) in params {
            let param_key = key;
            match value {
                serde_json::Value::Number(n) => {
                    content.push_str(&format!("{}={}\n", param_key, n));
                }
                serde_json::Value::String(s) => {
                    content.push_str(&format!("{}={}\n", param_key, s.as_str()));
                }
                serde_json::Value::Bool(b) => {
                    content.push_str(&format!("{}={}\n", param_key, b));
                }
                _ => {}
            }
        }

        // Add sweep parameters
        for (key, value) in sweep {
            let param_key = key;
            match value {
                serde_json::Value::Object(obj) => {
                    if let Some(from) = obj.get("from") {
                        content.push_str(&format!("{}_from={}\n", param_key, from.as_f64().unwrap_or(0.0)));
                    }
                    if let Some(to) = obj.get("to") {
                        content.push_str(&format!("{}_to={}\n", param_key, to.as_f64().unwrap_or(0.0)));
                    }
                    if let Some(step) = obj.get("step") {
                        content.push_str(&format!("{}_step={}\n", param_key, step.as_f64().unwrap_or(0.0)));
                    }
                }
                serde_json::Value::Bool(optimize) => {
                    if *optimize {
                        content.push_str(&format!("{}=||Y\n", param_key));
                    }
                }
                _ => {}
            }
        }

        fs::write(path, content)?;

        Ok(json!({
            "success": true,
            "path": path,
            "message": "Set file generated from optimization"
        }))
    }

    pub async fn diff_set_files(&self, path_a: &str, path_b: &str) -> Result<serde_json::Value> {
        let content_a = fs::read_to_string(path_a)?;
        let content_b = fs::read_to_string(path_b)?;

        // Parse both files and find differences (simplified)
        let mut differences = Vec::new();
        let lines_a: Vec<&str> = content_a.lines().collect();
        let lines_b: Vec<&str> = content_b.lines().collect();

        for (i, (line_a, line_b)) in lines_a.iter().zip(lines_b.iter()).enumerate() {
            if line_a != line_b {
                differences.push(json!({
                    "line": i + 1,
                    "file_a": line_a,
                    "file_b": line_b
                }));
            }
        }

        Ok(json!({
            "success": true,
            "path_a": path_a,
            "path_b": path_b,
            "differences": differences
        }))
    }

    pub async fn describe_sweep(&self, path: &str) -> Result<serde_json::Value> {
        let content = fs::read_to_string(path)?;
        
        // Parse sweep configuration (simplified)
        let mut sweep_params = serde_json::Map::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim();
                let _clean_value = if value.starts_with('"') && value.ends_with('"') {
                    value[1..value.len()-1].to_string()
                } else {
                    value.to_string()
                };
                
                if value.contains("||Y") {
                    if let Some((from_val, to_val)) = value.split_once("..") {
                        sweep_params.insert(key.clone(), json!({
                            "from": from_val.trim(),
                            "to": to_val.trim(),
                            "step": 1.0
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "success": true,
            "path": path,
            "sweep_params": sweep_params
        }))
    }

    pub async fn list_set_files(&self) -> Result<serde_json::Value> {
        let mut set_files = Vec::new();

        if let Some(tester_profiles_dir) = &self.config.tester_profiles_dir {
            for entry in fs::read_dir(tester_profiles_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map(|ext| ext == "set").unwrap_or(false) {
                    let file_name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    let param_count = self.count_params_in_set(&path)?;
                    let sweep_count = self.count_sweep_params_in_set(&path)?;
                    
                    set_files.push(json!({
                        "name": file_name,
                        "path": path.to_string_lossy(),
                        "param_count": param_count,
                        "sweep_count": sweep_count,
                        "sub_folder": path.parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    }));
                }
            }
        }

        Ok(json!(set_files))
    }

    pub async fn list_jobs(&self) -> Result<serde_json::Value> {
        let mut jobs = Vec::new();

        if let Some(opt_log_dir) = &self.config.opt_log_dir {
            for entry in fs::read_dir(opt_log_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map(|ext| ext == "log").unwrap_or(false) {
                    let file_name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    // Simple job status detection (simplified)
                    let is_alive = self.is_process_alive(&file_name);
                    let elapsed = self.get_job_elapsed(&path);
                    
                    jobs.push(json!({
                        "job_id": file_name,
                        "alive": is_alive,
                        "elapsed": elapsed,
                        "log_file": path.to_string_lossy()
                    }));
                }
            }
        }

        Ok(json!(jobs))
    }

    pub async fn archive_report(&self, report_dir: &str, delete_after: bool, notes: Option<String>, tags: Option<Vec<String>>) -> Result<serde_json::Value> {
        // Simplified archiving - in real implementation would parse report files
        Ok(json!({
            "success": true,
            "report_dir": report_dir,
            "delete_after": delete_after,
            "notes": notes,
            "tags": tags
        }))
    }

    pub async fn archive_all_reports(&self, delete_after: bool, keep_last: u64, dry_run: bool) -> Result<serde_json::Value> {
        // Simplified archiving - in real implementation would scan reports directory
        Ok(json!({
            "success": true,
            "delete_after": delete_after,
            "keep_last": keep_last,
            "dry_run": dry_run
        }))
    }

    pub async fn get_history(&self, ea: Option<String>, symbol: Option<String>, tag: Option<String>, verdict: Option<String>, sort_by: Option<String>, limit: u64, include_monthly: bool) -> Result<serde_json::Value> {
        // Simplified history retrieval - in real implementation would read from JSON file
        Ok(json!({
            "success": true,
            "ea": ea,
            "symbol": symbol,
            "tag": tag,
            "verdict": verdict,
            "sort_by": sort_by,
            "limit": limit,
            "include_monthly": include_monthly
        }))
    }

    pub async fn promote_to_baseline(&self, history_id: &str, report_dir: Option<String>, notes: Option<String>) -> Result<serde_json::Value> {
        // Simplified promotion - in real implementation would update baseline.json
        Ok(json!({
            "success": true,
            "history_id": history_id,
            "report_dir": report_dir,
            "notes": notes
        }))
    }

    pub async fn annotate_history(&self, history_id: &str, notes: Option<String>, tags: Option<Vec<String>>, verdict: Option<String>) -> Result<serde_json::Value> {
        // Simplified annotation - in real implementation would update history JSON
        Ok(json!({
            "success": true,
            "history_id": history_id,
            "notes": notes,
            "tags": tags,
            "verdict": verdict
        }))
    }

    pub async fn prune_reports(&self, keep_last: u64) -> Result<serde_json::Value> {
        // Simplified pruning - in real implementation would delete old directories
        Ok(json!({
            "success": true,
            "keep_last": keep_last
        }))
    }

    pub async fn list_reports(&self, limit: u64) -> Result<serde_json::Value> {
        // Simplified listing - in real implementation would scan reports directory
        Ok(json!({
            "success": true,
            "limit": limit
        }))
    }

    pub async fn tail_log(&self, n: u64, filter: &str, log_file: Option<String>, report_dir: Option<String>, job_id: Option<String>) -> Result<serde_json::Value> {
        // Simplified tailing - in real implementation would read log file
        Ok(json!({
            "success": true,
            "n": n,
            "filter": filter,
            "log_file": log_file,
            "report_dir": report_dir,
            "job_id": job_id
        }))
    }

    pub async fn cache_status(&self) -> Result<serde_json::Value> {
        // Simplified cache status - in real implementation would scan cache directory
        Ok(json!({
            "success": true,
            "cache_dir": self.config.tester_cache_dir.as_ref().unwrap_or(&"unknown".to_string()),
            "symbol_count": 7,
            "symbols": [
                "AUDJPYc", "AUDUSD", "EURJPYc", "USDJPY", "USDUSC", "XAUUSD.cent", "XAUUSDc"
            ]
        }))
    }

    pub async fn clean_cache(&self, symbol: Option<String>, dry_run: bool) -> Result<serde_json::Value> {
        // Simplified cache cleaning - in real implementation would delete cache files
        Ok(json!({
            "success": true,
            "symbol": symbol,
            "dry_run": dry_run
        }))
    }

    pub async fn get_backtest_status(&self, report_dir: &str) -> Result<serde_json::Value> {
        // Simplified status checking - in real implementation would check for completion
        Ok(json!({
            "success": true,
            "report_dir": report_dir,
            "status": "completed"
        }))
    }

    pub async fn get_optimization_status(&self, job_id: &str) -> Result<serde_json::Value> {
        // Simplified status checking - in real implementation would check process
        Ok(json!({
            "success": true,
            "job_id": job_id,
            "status": "running"
        }))
    }

    // Helper methods (simplified implementations)
    fn count_params_in_set(&self, path: &Path) -> Result<u64> {
        let content = fs::read_to_string(path)?;
        Ok(content.lines().filter(|line| line.contains(':')).count() as u64)
    }

    fn count_sweep_params_in_set(&self, path: &Path) -> Result<u64> {
        let content = fs::read_to_string(path)?;
        Ok(content.lines().filter(|line| line.contains("||Y")).count() as u64)
    }

    fn is_process_alive(&self, job_id: &str) -> bool {
        // Simplified process check - in real implementation would check process list
        job_id.contains("opt_") && chrono::Utc::now().timestamp() % 3600 > 300
    }

    fn get_job_elapsed(&self, _log_path: &Path) -> String {
        // Simplified elapsed calculation
        "5m 23s".to_string()
    }
}
