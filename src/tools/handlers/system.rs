use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use crate::models::Config;

// ── Update helpers ────────────────────────────────────────────────────────────

fn platform_tag() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))] return "macos-aarch64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]  return "macos-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]  return "linux-x86_64";
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))] return "unsupported";
}

fn semver_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let mut p = s.trim_start_matches('v').splitn(3, '.');
        let ma = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let mi = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let pa = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        (ma, mi, pa)
    };
    parse(latest) > parse(current)
}

/// Fetch the latest release tag from GitHub API (5 s timeout via curl).
/// Returns the version string without the leading "v", or None on failure.
pub(super) async fn fetch_latest_version() -> Option<String> {
    let output = tokio::process::Command::new("curl")
        .args([
            "-sf", "--max-time", "5",
            "-H", "Accept: application/vnd.github.v3+json",
            "-H", "User-Agent: mt5-quant-updater",
            "https://api.github.com/repos/masdevid/mt5-quant/releases/latest",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() { return None; }

    let body: Value = serde_json::from_slice(&output.stdout).ok()?;
    body.get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
}

fn ok_response(data: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": data.to_string() }], "isError": false })
}

fn err_response(msg: impl std::fmt::Display) -> Value {
    json!({ "content": [{ "type": "text", "text": msg.to_string() }], "isError": true })
}

// ── Update tool handlers ──────────────────────────────────────────────────────

pub async fn handle_check_update(_config: &Config) -> Result<Value> {
    let current = env!("CARGO_PKG_VERSION");

    // Use cached background-check result if available; otherwise fetch now.
    let latest_opt = match super::LATEST_VERSION.get() {
        Some(v) => v.clone(),
        None => fetch_latest_version().await,
    };

    let Some(latest) = latest_opt else {
        return Ok(ok_response(json!({
            "current_version": current,
            "update_available": false,
            "error": "Could not reach GitHub API — check network connectivity",
        })));
    };

    let update_available = semver_newer(&latest, current);
    Ok(ok_response(json!({
        "current_version": current,
        "latest_version": latest,
        "update_available": update_available,
        "hint": if update_available {
            format!("Run the `update` tool to install v{latest}")
        } else {
            "You are on the latest version".to_string()
        },
    })))
}

pub async fn handle_update(_config: &Config) -> Result<Value> {
    let current = env!("CARGO_PKG_VERSION");

    let latest = match super::LATEST_VERSION.get().and_then(|v| v.as_deref()) {
        Some(v) => v.to_string(),
        None => match fetch_latest_version().await {
            Some(v) => v,
            None => return Ok(err_response(
                r#"{"success":false,"error":"Could not determine latest version — check network"}"#
            )),
        },
    };

    if !semver_newer(&latest, current) {
        return Ok(ok_response(json!({
            "up_to_date": true,
            "version": current,
        })));
    }

    let tag = platform_tag();
    if tag == "unsupported" {
        return Ok(err_response(
            r#"{"success":false,"error":"Auto-update not supported on this platform — build from source"}"#
        ));
    }

    let url = format!(
        "https://github.com/masdevid/mt5-quant/releases/download/v{latest}/mcp-mt5-quant-{tag}.tar.gz"
    );

    // Download tarball to a temp file
    let tmp_tar = tempfile::NamedTempFile::new()?;
    let dl = tokio::process::Command::new("curl")
        .args(["-sfL", "--max-time", "120",
               "-o", tmp_tar.path().to_str().unwrap_or_default(),
               &url])
        .status()
        .await?;

    if !dl.success() {
        return Ok(err_response(format!(
            r#"{{"success":false,"error":"Download failed","url":"{}"}}"#, url
        )));
    }

    // Extract binary (tarball root dir is mcp-mt5-quant-{platform}/)
    let tmp_dir = tempfile::tempdir()?;
    let extract = tokio::process::Command::new("tar")
        .args(["-xzf", tmp_tar.path().to_str().unwrap_or_default(),
               "-C", tmp_dir.path().to_str().unwrap_or_default(),
               "--strip-components=1"])
        .status()
        .await?;

    if !extract.success() {
        return Ok(err_response(r#"{"success":false,"error":"Failed to extract archive"}"#));
    }

    let new_bin = tmp_dir.path().join("mt5-quant");
    if !new_bin.exists() {
        return Ok(err_response(r#"{"success":false,"error":"Binary not found in archive"}"#));
    }

    // Atomic replace: write to sibling .tmp, then rename (safe on same FS)
    let current_exe = std::env::current_exe()?;
    let tmp_dest = current_exe.with_extension("update_tmp");
    std::fs::copy(&new_bin, &tmp_dest)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_dest, std::fs::Permissions::from_mode(0o755))?;
    }

    std::fs::rename(&tmp_dest, &current_exe)?;

    Ok(ok_response(json!({
        "success": true,
        "previous_version": current,
        "updated_to": latest,
        "binary": current_exe.to_string_lossy(),
        "hint": format!("Updated to v{latest}. Restart the MCP connection to load the new binary."),
    })))
}

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
    // Get active account info
    let current_account = config.current_account();
    let active_server = current_account.as_ref().map(|a| a.server.clone());
    
    // Get all available servers for reference
    let all_servers = config.available_servers();
    
    // Get symbols for active server (or all if no active account)
    let symbols = config.discover_symbols_for_active_account();
    
    let hint = if symbols.is_empty() {
        if active_server.is_some() {
            "No history data found for the active account's server. Open MT5 and download tick data for the symbols you want to backtest."
        } else {
            "No history data found. Open MT5 and download tick data for the symbols you want to backtest."
        }
    } else {
        "These symbols have local tick history and can be used for backtesting."
    };
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": symbols.len(),
            "symbols": symbols,
            "active_account": current_account.map(|a| json!({
                "login": a.login,
                "server": a.server
            })),
            "active_server": active_server,
            "available_servers": all_servers,
            "hint": hint,
        }).to_string() }],
        "isError": false
    }))
}

/// Get active MT5 account info with available symbols for pre-flight checks
pub async fn handle_get_active_account(config: &Config) -> Result<Value> {
    let current_account = config.current_account();
    let active_server = current_account.as_ref().map(|a| a.server.clone());
    
    // Get all available servers
    let all_servers = config.available_servers();
    
    // Get symbols for active server (or all if no active account)
    let symbols = config.discover_symbols_for_active_account();
    
    // Determine readiness for backtesting
    let ready_for_backtest = current_account.is_some() && !symbols.is_empty();
    
    let hint = if current_account.is_none() {
        "No active MT5 account detected. Open MT5 and login to an account first."
    } else if symbols.is_empty() {
        "Active account found but no symbol history data. Download tick data in MT5 Strategy Tester."
    } else {
        "Ready for backtesting. Use these symbols with run_backtest."
    };
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "ready_for_backtest": ready_for_backtest,
            "account": current_account.map(|a| json!({
                "login": a.login,
                "server": a.server
            })),
            "server": active_server,
            "available_servers": all_servers,
            "symbols": symbols,
            "symbol_count": symbols.len(),
            "hint": hint,
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
