use anyhow::{anyhow, Result};
use std::os::unix::fs::PermissionsExt;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tokio::process::Command as AsyncCommand;

use crate::config::Config;

#[derive(Debug)]
pub struct Mt5Manager {
    config: Config,
}

impl Mt5Manager {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn verify_setup(&self) -> Result<serde_json::Value> {
        let mut checks = HashMap::new();
        let mut all_ok = true;

        // Check config file
        let config_path = Config::get_config_path();
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
        let expert = params.get("expert")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("expert parameter is required"))?;

        let symbol = params.get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("XAUUSD");

        let timeframe = params.get("timeframe")
            .and_then(|v| v.as_str())
            .unwrap_or("M5");

        let deposit = params.get("deposit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10000);

        let from_date = params.get("from_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("from_date parameter is required"))?;

        let to_date = params.get("to_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("to_date parameter is required"))?;

        // Create report directory
        let report_name = format!(
            "{}_{}_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            expert,
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
            &format!("/expert:{}", expert),
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

    pub async fn compile_ea(&self, expert_path: &str) -> Result<serde_json::Value> {
        let path = Path::new(expert_path);
        if !path.exists() {
            return Err(anyhow!("Expert file not found: {}", expert_path));
        }

        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if extension != "mq5" {
            return Err(anyhow!("File must have .mq5 extension"));
        }

        // Build MetaEditor command
        let mut cmd = AsyncCommand::new(&self.config.wine_executable.as_ref().unwrap());
        cmd.args([
            "metaeditor64.exe",
            &format!("/compile:{}", expert_path),
            "/close",
        ]);

        cmd.current_dir(Path::new(&self.config.terminal_dir.as_ref().unwrap()));

        let output = cmd.output().await?;

        if !output.status.success() {
            return Err(anyhow!("Compilation failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        Ok(json!({
            "success": true,
            "message": "Expert compiled successfully",
            "expert_path": expert_path
        }))
    }
}
