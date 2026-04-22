use anyhow::Result;
use serde_json::{json, Value};
use walkdir::WalkDir;
use crate::compile::MqlCompiler;
use crate::models::Config;

const BUILTIN_INDICATORS: &[&str] = &[
    "Accelerator", "Accumulation", "ADX", "Alligator", "AO", "ATR",
    "Bands", "Bears", "Bulls", "CCI", "DeMarker", "Envelopes", "Force",
    "Fractals", "Gator", "Ichimoku", "MA", "MACD", "MFI", "Momentum",
    "OBV", "OsMA", "RSI", "RVI", "SAR", "StdDev", "Stochastic", "WPR",
];

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Walk a MQL directory and return file entries matching an optional filter.
/// `type_label` is included as a `"type"` field when provided (e.g. "custom").
fn scan_mql_dir(dir: Option<&String>, filter: Option<&str>, type_label: Option<&str>) -> Vec<Value> {
    let Some(dir) = dir else { return Vec::new() };
    let filter_lower = filter.map(|f| f.to_lowercase());

    let mut items: Vec<Value> = WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_stem()?.to_string_lossy().into_owned();
            if let Some(ref f) = filter_lower {
                if !name.to_lowercase().contains(f.as_str()) {
                    return None;
                }
            }
            let is_compiled = path.extension().map(|ext| ext == "ex5").unwrap_or(false);
            let mut obj = json!({
                "name": name,
                "compiled": is_compiled,
                "path": path.to_string_lossy().as_ref(),
            });
            if let Some(t) = type_label {
                obj["type"] = json!(t);
            }
            Some(obj)
        })
        .collect();

    items.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    items
}

fn ok_response(data: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": data.to_string() }],
        "isError": false
    })
}

fn err_response(msg: impl std::fmt::Display) -> Value {
    json!({
        "content": [{ "type": "text", "text": msg.to_string() }],
        "isError": true
    })
}

/// Shared implementation for copy_indicator_to_project / copy_script_to_project.
/// `default_fallback` is used as the stem when the source has no filename.
fn copy_mql_to_project(config: &Config, args: &Value, default_fallback: &str) -> Result<Value> {
    use std::path::PathBuf;

    let source_path = args.get("source_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("source_path is required"))?;

    let target_name = args.get("target_name").and_then(|v| v.as_str());

    let project_dir = config.project_dir.as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project directory"))?;

    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Ok(err_response(
            serde_json::to_string(&json!({ "success": false, "error": format!("Source file not found: {}", source_path) })).unwrap_or_default()
        ));
    }

    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("mq5");
    let target_filename = match target_name {
        Some(name) => format!("{}.{}", name, ext),
        None => source.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("{}.{}", default_fallback, ext)),
    };

    let destination = project_dir.join(&target_filename);

    match std::fs::copy(&source, &destination) {
        Ok(bytes) => Ok(ok_response(json!({
            "success": true,
            "source": source_path,
            "destination": destination.to_string_lossy().as_ref(),
            "bytes_copied": bytes,
        }))),
        Err(e) => Ok(err_response(
            serde_json::to_string(&json!({ "success": false, "error": format!("Failed to copy file: {}", e) })).unwrap_or_default()
        )),
    }
}

// ── Public handlers ───────────────────────────────────────────────────────────

pub async fn handle_list_experts(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    let experts = scan_mql_dir(config.experts_dir.as_ref(), filter, None);
    Ok(ok_response(json!({
        "success": true,
        "count": experts.len(),
        "experts": experts,
    })))
}

pub async fn handle_search_experts(config: &Config, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    let matches = scan_mql_dir(config.experts_dir.as_ref(), Some(pattern), None);
    Ok(ok_response(json!({
        "success": true,
        "pattern": pattern,
        "count": matches.len(),
        "matches": matches,
    })))
}

pub async fn handle_list_indicators(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    let include_builtin = args.get("include_builtin").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut indicators = scan_mql_dir(config.indicators_dir.as_ref(), filter, Some("custom"));

    if include_builtin {
        let filter_lower = filter.map(|f| f.to_lowercase());
        for &name in BUILTIN_INDICATORS {
            if filter_lower.as_ref().map(|f| name.to_lowercase().contains(f.as_str())).unwrap_or(true) {
                indicators.push(json!({ "name": name, "compiled": true, "type": "builtin", "path": null }));
            }
        }
        indicators.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    }

    Ok(ok_response(json!({
        "success": true,
        "count": indicators.len(),
        "indicators": indicators,
        "custom_dir": config.indicators_dir.clone(),
    })))
}

pub async fn handle_list_scripts(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    let scripts = scan_mql_dir(config.scripts_dir.as_ref(), filter, None);
    Ok(ok_response(json!({
        "success": true,
        "count": scripts.len(),
        "scripts": scripts,
        "scripts_dir": config.scripts_dir.clone(),
    })))
}

pub async fn handle_search_indicators(config: &Config, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    let include_builtin = args.get("include_builtin").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut matches = scan_mql_dir(config.indicators_dir.as_ref(), Some(pattern), Some("custom"));

    if include_builtin {
        let pattern_lower = pattern.to_lowercase();
        for &name in BUILTIN_INDICATORS {
            if name.to_lowercase().contains(&pattern_lower) {
                matches.push(json!({ "name": name, "path": null, "type": "builtin", "compiled": true }));
            }
        }
        matches.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    }

    Ok(ok_response(json!({
        "success": true,
        "pattern": pattern,
        "count": matches.len(),
        "matches": matches,
    })))
}

pub async fn handle_search_scripts(config: &Config, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    let matches = scan_mql_dir(config.scripts_dir.as_ref(), Some(pattern), None);
    Ok(ok_response(json!({
        "success": true,
        "pattern": pattern,
        "count": matches.len(),
        "matches": matches,
    })))
}

pub async fn handle_compile_ea(config: &Config, args: &Value) -> Result<Value> {
    use std::path::PathBuf;

    let resolved_path: String = if let Some(p) = args.get("expert_path").and_then(|v| v.as_str()) {
        p.to_string()
    } else if let Some(name) = args.get("expert").and_then(|v| v.as_str()) {
        let mut candidates = vec![PathBuf::from(name).with_extension("mq5")];
        if let Some(experts_dir) = &config.experts_dir {
            candidates.push(PathBuf::from(experts_dir).join(name).join(format!("{}.mq5", name)));
            candidates.push(PathBuf::from(experts_dir).join(format!("{}.mq5", name)));
        }
        match candidates.into_iter().find(|p| p.exists()) {
            Some(p) => p.to_string_lossy().to_string(),
            None => return Ok(err_response(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Cannot find {}.mq5 in MT5 Experts dir or current directory", name),
                })).unwrap_or_default()
            )),
        }
    } else {
        return Err(anyhow::anyhow!("Either 'expert' or 'expert_path' is required"));
    };

    let compiler = MqlCompiler::new(config.clone());
    match compiler.compile(&resolved_path).await {
        Ok(result) => Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": result.success,
                "binary_path": result.ex5_path.map(|p| p.to_string_lossy().to_string()),
                "binary_size_bytes": result.binary_size,
                "files_synced": result.files_synced,
                "warnings": result.warnings.len(),
                "errors": result.errors.len(),
                "error_list": result.errors,
                "warning_list": result.warnings,
            }).to_string() }],
            "isError": !result.success
        })),
        Err(e) => Ok(err_response(
            serde_json::to_string(&json!({ "success": false, "error": format!("Compilation failed: {}", e) })).unwrap_or_default()
        )),
    }
}

pub async fn handle_copy_indicator_to_project(config: &Config, args: &Value) -> Result<Value> {
    copy_mql_to_project(config, args, "indicator")
}

pub async fn handle_copy_script_to_project(config: &Config, args: &Value) -> Result<Value> {
    copy_mql_to_project(config, args, "script")
}
