use anyhow::Result;
use serde_json::{json, Value};
use walkdir::WalkDir;
use crate::compile::MqlCompiler;
use crate::models::Config;

pub async fn handle_list_experts(config: &Config, args: &Value) -> Result<Value> {
    let filter = args.get("filter").and_then(|v| v.as_str());
    
    let mut experts = Vec::new();
    
    if let Some(experts_dir) = &config.experts_dir {
        for entry in WalkDir::new(experts_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
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

pub async fn handle_search_experts(config: &Config, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    
    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();
    
    if let Some(experts_dir) = &config.experts_dir {
        for entry in WalkDir::new(experts_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    if name_str.to_lowercase().contains(&pattern_lower) {
                        let is_compiled = path.extension()
                            .map(|e| e == "ex5")
                            .unwrap_or(false);
                        matches.push(json!({
                            "name": name_str,
                            "path": path.to_string_lossy().to_string(),
                            "compiled": is_compiled,
                        }));
                    }
                }
            }
        }
    }
    
    matches.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "pattern": pattern,
            "count": matches.len(),
            "matches": matches,
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
        for entry in WalkDir::new(indicators_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
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
        for entry in WalkDir::new(scripts_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
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
    
    match compiler.compile(&expert_path).await {
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

pub async fn handle_search_indicators(config: &Config, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    
    let include_builtin = args.get("include_builtin").and_then(|v| v.as_bool()).unwrap_or(false);
    let pattern_lower = pattern.to_lowercase();
    
    let mut matches = Vec::new();
    
    // Search custom indicators recursively
    if let Some(indicators_dir) = &config.indicators_dir {
        for entry in WalkDir::new(indicators_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    if name_str.to_lowercase().contains(&pattern_lower) {
                        let is_compiled = path.extension()
                            .map(|e| e == "ex5")
                            .unwrap_or(false);
                        matches.push(json!({
                            "name": name_str,
                            "path": path.to_string_lossy().to_string(),
                            "type": "custom",
                            "compiled": is_compiled,
                        }));
                    }
                }
            }
        }
    }
    
    // Search built-in indicators if requested
    if include_builtin {
        let builtin = vec![
            "Accelerator", "Accumulation", "ADX", "Alligator", "AO", "ATR",
            "Bands", "Bears", "Bulls", "CCI", "DeMarker", "Envelopes", "Force",
            "Fractals", "Gator", "Ichimoku", "MA", "MACD", "MFI", "Momentum",
            "OBV", "OsMA", "RSI", "RVI", "SAR", "StdDev", "Stochastic", "WPR",
        ];
        for name in builtin {
            if name.to_lowercase().contains(&pattern_lower) {
                matches.push(json!({
                    "name": name,
                    "path": null,
                    "type": "builtin",
                    "compiled": true,
                }));
            }
        }
    }
    
    matches.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "pattern": pattern,
            "count": matches.len(),
            "matches": matches,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_scripts(config: &Config, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    
    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();
    
    if let Some(scripts_dir) = &config.scripts_dir {
        for entry in WalkDir::new(scripts_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();
                    if name_str.to_lowercase().contains(&pattern_lower) {
                        let is_compiled = path.extension()
                            .map(|e| e == "ex5")
                            .unwrap_or(false);
                        matches.push(json!({
                            "name": name_str,
                            "path": path.to_string_lossy().to_string(),
                            "compiled": is_compiled,
                        }));
                    }
                }
            }
        }
    }
    
    matches.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    
    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "pattern": pattern,
            "count": matches.len(),
            "matches": matches,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_copy_indicator_to_project(config: &Config, args: &Value) -> Result<Value> {
    use std::path::PathBuf;
    
    let source_path = args.get("source_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("source_path is required"))?;
    
    let target_name = args.get("target_name").and_then(|v| v.as_str());
    
    // Determine project directory
    let project_dir = config.project_dir.as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project directory"))?;
    
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": false,
                "error": format!("Source file not found: {}", source_path),
            }).to_string() }],
            "isError": true
        }));
    }
    
    // Get extension
    let ext = source.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mq5");
    
    // Determine target name
    let target_filename = match target_name {
        Some(name) => format!("{}.{}", name, ext),
        None => source.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("indicator.{}", ext)),
    };
    
    let destination = project_dir.join(&target_filename);
    
    // Copy file
    match std::fs::copy(&source, &destination) {
        Ok(bytes) => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": true,
                    "source": source_path,
                    "destination": destination.to_string_lossy().to_string(),
                    "bytes_copied": bytes,
                }).to_string() }],
                "isError": false
            }))
        }
        Err(e) => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "error": format!("Failed to copy file: {}", e),
                }).to_string() }],
                "isError": true
            }))
        }
    }
}

pub async fn handle_copy_script_to_project(config: &Config, args: &Value) -> Result<Value> {
    use std::path::PathBuf;
    
    let source_path = args.get("source_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("source_path is required"))?;
    
    let target_name = args.get("target_name").and_then(|v| v.as_str());
    
    // Determine project directory
    let project_dir = config.project_dir.as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project directory"))?;
    
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Ok(json!({
            "content": [{ "type": "text", "text": json!({
                "success": false,
                "error": format!("Source file not found: {}", source_path),
            }).to_string() }],
            "isError": true
        }));
    }
    
    // Get extension
    let ext = source.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mq5");
    
    // Determine target name
    let target_filename = match target_name {
        Some(name) => format!("{}.{}", name, ext),
        None => source.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("script.{}", ext)),
    };
    
    let destination = project_dir.join(&target_filename);
    
    // Copy file
    match std::fs::copy(&source, &destination) {
        Ok(bytes) => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": true,
                    "source": source_path,
                    "destination": destination.to_string_lossy().to_string(),
                    "bytes_copied": bytes,
                }).to_string() }],
                "isError": false
            }))
        }
        Err(e) => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "error": format!("Failed to copy file: {}", e),
                }).to_string() }],
                "isError": true
            }))
        }
    }
}
