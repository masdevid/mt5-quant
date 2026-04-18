use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use crate::compile::MqlCompiler;
use crate::models::Config;

pub async fn handle_list_experts(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    
    let mut experts = Vec::new();
    
    if let Some(experts_dir) = &config.experts_dir {
        if let Ok(entries) = fs::read_dir(experts_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    let is_compiled = path.extension()
                        .map(|e| e == "ex5")
                        .unwrap_or(false);
                    
                    if let Some(filter_str) = filter {
                        if !name_str.to_lowercase().contains(&filter_str.to_lowercase()) {
                            continue;
                        }
                    }
                    
                    experts.push(json!({
                        "name": name_str,
                        "compiled": is_compiled,
                        "path": path.to_string_lossy().to_string(),
                    }));
                }
            }
        }
    }
    
    experts.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": experts.len(),
            "experts": experts,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_list_indicators(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    let include_builtin = args.get("include_builtin").and_then(|v| v.as_bool()).unwrap_or(false);
    
    let mut indicators = Vec::new();
    
    // List custom indicators
    if let Some(indicators_dir) = &config.indicators_dir {
        if let Ok(entries) = fs::read_dir(indicators_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    let is_compiled = path.extension()
                        .map(|e| e == "ex5")
                        .unwrap_or(false);
                    
                    if let Some(filter_str) = filter {
                        if !name_str.to_lowercase().contains(&filter_str.to_lowercase()) {
                            continue;
                        }
                    }
                    
                    indicators.push(json!({
                        "name": name_str,
                        "compiled": is_compiled,
                        "type": "custom",
                        "path": path.to_string_lossy().to_string(),
                    }));
                }
            }
        }
    }
    
    // Add built-in indicators if requested
    if include_builtin {
        let builtin = vec![
            "Accelerator", "Accumulation", "ADX", "Alligator", "AO", "ATR",
            "Bands", "Bears", "Bulls", "CCI", "DeMarker", "Envelopes", "Force",
            "Fractals", "Gator", "Ichimoku", "MA", "MACD", "MFI", "Momentum",
            "OBV", "OsMA", "RSI", "RVI", "SAR", "StdDev", "Stochastic", "WPR",
        ];
        for name in builtin {
            if filter.map(|f| name.to_lowercase().contains(&f.to_lowercase())).unwrap_or(true) {
                indicators.push(json!({
                    "name": name,
                    "compiled": true,
                    "type": "builtin",
                    "path": null,
                }));
            }
        }
    }
    
    indicators.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": indicators.len(),
            "indicators": indicators,
            "custom_dir": config.indicators_dir.clone(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_list_scripts(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    
    let mut scripts = Vec::new();
    
    if let Some(scripts_dir) = &config.scripts_dir {
        if let Ok(entries) = fs::read_dir(scripts_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    let is_compiled = path.extension()
                        .map(|e| e == "ex5")
                        .unwrap_or(false);
                    
                    if let Some(filter_str) = filter {
                        if !name_str.to_lowercase().contains(&filter_str.to_lowercase()) {
                            continue;
                        }
                    }
                    
                    scripts.push(json!({
                        "name": name_str,
                        "compiled": is_compiled,
                        "path": path.to_string_lossy().to_string(),
                    }));
                }
            }
        }
    }
    
    scripts.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "count": scripts.len(),
            "scripts": scripts,
            "scripts_dir": config.scripts_dir.clone(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_compile_ea(config: &Config, args: &Value) -> Result<Value> {
    use std::path::PathBuf;

    let resolved_path: String = if let Some(p) = args.get("expert_path").and_then(|v| v.as_str()) {
        p.to_string()
    } else if let Some(name) = args.get("expert").and_then(|v| v.as_str()) {
        let mut candidates = vec![
            PathBuf::from(name).with_extension("mq5"),
        ];
        if let Some(experts_dir) = &config.experts_dir {
            candidates.push(PathBuf::from(experts_dir).join(name).join(format!("{}.mq5", name)));
            candidates.push(PathBuf::from(experts_dir).join(format!("{}.mq5", name)));
        }
        match candidates.into_iter().find(|p| p.exists()) {
            Some(p) => p.to_string_lossy().to_string(),
            None => return Ok(serde_json::json!({
                "content": [{ "type": "text", "text": serde_json::json!({
                    "success": false,
                    "error": format!("Cannot find {}.mq5 in MT5 Experts dir or current directory", name),
                }).to_string() }],
                "isError": true
            })),
        }
    } else {
        return Err(anyhow::anyhow!("Either 'expert' or 'expert_path' is required"));
    };

    let compiler = MqlCompiler::new(config.clone());
    let expert_path = resolved_path.as_str();
    
    match compiler.compile(&expert_path) {
        Ok(result) => {
            Ok(json!({
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
            }))
        }
        Err(e) => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "error": format!("Compilation failed: {}", e),
                }).to_string() }],
                "isError": true
            }))
        }
    }
}
