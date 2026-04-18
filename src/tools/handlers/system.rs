use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use crate::models::Config;

pub async fn handle_verify_setup(config: &Config) -> Result<Value> {
    let mut checks = serde_json::Map::new();
    let mut all_ok = true;

    let config_path = Config::writable_config_path();
    checks.insert("config_file".into(), json!({
        "ok": config_path.exists(),
        "path": config_path.to_string_lossy()
    }));

    let check = |v: &Option<String>, is_dir: bool| -> Value {
        match v {
            None => json!({ "ok": false, "detail": "not set" }),
            Some(p) => {
                let ok = if is_dir { Path::new(p).is_dir() } else { Path::new(p).exists() };
                json!({ "ok": ok, "detail": p })
            }
        }
    };

    let wine_ok = config.wine_executable.as_ref()
        .map(|p| Path::new(p).exists()).unwrap_or(false);
    let term_ok = config.terminal_dir.as_ref()
        .map(|p| Path::new(p).is_dir()).unwrap_or(false);

    if !wine_ok || !term_ok { all_ok = false; }

    checks.insert("wine_executable".into(), check(&config.wine_executable, false));
    checks.insert("terminal_dir".into(), check(&config.terminal_dir, true));
    checks.insert("experts_dir".into(), check(&config.experts_dir, true));
    checks.insert("indicators_dir".into(), check(&config.indicators_dir, true));
    checks.insert("scripts_dir".into(), check(&config.scripts_dir, true));
    checks.insert("tester_profiles_dir".into(), check(&config.tester_profiles_dir, true));
    checks.insert("display_mode".into(), json!(config.display_mode));
    checks.insert("reports_dir".into(), json!(config.reports_dir().to_string_lossy().to_string()));
    checks.insert("db_path".into(), json!(Config::db_path().to_string_lossy().to_string()));

    let hint = if all_ok {
        "Environment fully configured and ready".into()
    } else if !config_path.exists() {
        format!("Auto-discovery will run on next request. Config will be written to {}", config_path.display())
    } else {
        format!("Fix missing paths in {}", config_path.display())
    };

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "all_ok": all_ok,
            "config_path": config_path.to_string_lossy(),
            "checks": checks,
            "hint": hint,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_list_symbols(config: &Config) -> Result<Value> {
    let symbols = config.discover_symbols();
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": symbols.len(),
            "symbols": symbols,
            "hint": if symbols.is_empty() {
                "No history data found. Open MT5 and download tick data for the symbols you want to backtest."
            } else {
                "These symbols have local tick history and can be used for backtesting."
            }
        }).to_string() }],
        "isError": false
    }))
}

// OS Detection structs and healthcheck
#[derive(Debug)]
struct OsInfo {
    platform: String,
    arch: String,
    name: String,
    is_macos: bool,
    is_linux: bool,
}

#[derive(Debug)]
struct ConfigStatus {
    config_exists: bool,
    config_path: String,
    wine_found: bool,
    wine_path: Option<String>,
    mt5_dir_found: bool,
    mt5_dir: Option<String>,
    experts_dir_found: bool,
    indicators_dir_found: bool,
    scripts_dir_found: bool,
    tester_profiles_found: bool,
}

pub async fn handle_healthcheck(config: &Config, args: &Value) -> Result<Value> {
    let detailed = args.get("detailed").and_then(|v| v.as_bool()).unwrap_or(false);
    
    let os_info = detect_os();
    let config_status = validate_configuration(config).await;
    
    let mut healthy = true;
    let mut issues = Vec::new();
    
    if !config_status.config_exists {
        healthy = false;
        issues.push("Configuration file not found - run setup to configure");
    }
    if !config_status.wine_found {
        healthy = false;
        issues.push("Wine/CrossOver not found - required for MT5 execution");
    }
    if !config_status.mt5_dir_found {
        healthy = false;
        issues.push("MT5 directory not found - check installation");
    }
    
    let mut response = json!({
        "success": true,
        "healthy": healthy,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "os": {
            "platform": os_info.platform,
            "arch": os_info.arch,
            "name": os_info.name,
            "is_macos": os_info.is_macos,
            "is_linux": os_info.is_linux,
        },
        "configuration": {
            "config_exists": config_status.config_exists,
            "config_path": config_status.config_path,
            "wine_found": config_status.wine_found,
            "wine_path": config_status.wine_path,
            "mt5_dir_found": config_status.mt5_dir_found,
            "mt5_dir": config_status.mt5_dir,
            "experts_dir_found": config_status.experts_dir_found,
            "indicators_dir_found": config_status.indicators_dir_found,
            "scripts_dir_found": config_status.scripts_dir_found,
            "tester_profiles_found": config_status.tester_profiles_found,
        },
        "issues": issues,
    });
    
    if detailed {
        response["detailed"] = json!({
            "rust_version": get_rust_version(),
            "exe_path": std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            "working_dir": std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            "env_vars": {
                "DISPLAY": std::env::var("DISPLAY").ok(),
                "WINEPREFIX": std::env::var("WINEPREFIX").ok(),
                "HOME": std::env::var("HOME").ok(),
            },
        });
    }
    
    Ok(json!({
        "content": [{ "type": "text", "text": response.to_string() }],
        "isError": false
    }))
}

fn detect_os() -> OsInfo {
    let platform = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    
    let is_macos = platform == "macos";
    let is_linux = platform == "linux";
    
    let name = if is_macos {
        get_macos_version().unwrap_or_else(|| "macOS".to_string())
    } else if is_linux {
        get_linux_distro().unwrap_or_else(|| "Linux".to_string())
    } else {
        platform.clone()
    };
    
    OsInfo {
        platform,
        arch,
        name,
        is_macos,
        is_linux,
    }
}

fn get_macos_version() -> Option<String> {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| format!("macOS {}", s.trim()))
}

fn get_linux_distro() -> Option<String> {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content.lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .map(|l| l.replace("PRETTY_NAME=", "").trim_matches('"').to_string())
        })
}

fn get_rust_version() -> Option<String> {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

async fn validate_configuration(config: &Config) -> ConfigStatus {
    let config_path = Config::writable_config_path();
    let config_exists = config_path.exists();
    
    let wine_found = config.wine_executable.as_ref()
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);
    let wine_path = config.wine_executable.clone();
    
    let mt5_dir_found = config.terminal_dir.as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);
    let mt5_dir = config.terminal_dir.clone();
    
    let experts_dir_found = config.experts_dir.as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);
    
    let indicators_dir_found = config.indicators_dir.as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);
    
    let scripts_dir_found = config.scripts_dir.as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);
    
    let tester_profiles_found = config.tester_profiles_dir.as_ref()
        .map(|p| Path::new(p).is_dir())
        .unwrap_or(false);
    
    ConfigStatus {
        config_exists,
        config_path: config_path.to_string_lossy().to_string(),
        wine_found,
        wine_path,
        mt5_dir_found,
        mt5_dir,
        experts_dir_found,
        indicators_dir_found,
        scripts_dir_found,
        tester_profiles_found,
    }
}
