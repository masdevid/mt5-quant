use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use crate::models::Config;

pub async fn handle_read_set_file(args: &Value) -> Result<Value> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;

    let content = fs::read_to_string(path)?;
    let mut params = serde_json::Map::new();

    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            
            if value.contains("||Y") {
                let parts: Vec<&str> = value.split("||").collect();
                if parts.len() >= 5 {
                    params.insert(key.to_string(), json!({
                        "value": parts[0],
                        "from": parts[1],
                        "step": parts[2],
                        "to": parts[3],
                        "optimize": true,
                    }));
                }
            } else {
                params.insert(key.to_string(), json!({ "value": value, "optimize": false }));
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "path": path,
            "parameters": params,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_write_set_file(args: &Value) -> Result<Value> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;

    let params = args.get("parameters")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("parameters object is required"))?;

    let mut lines = Vec::new();
    for (key, value) in params {
        if let Some(obj) = value.as_object() {
            if obj.get("optimize").and_then(|v| v.as_bool()).unwrap_or(false) {
                let from_val = obj.get("from").and_then(|v| v.as_str()).unwrap_or("0");
                let step = obj.get("step").and_then(|v| v.as_str()).unwrap_or("1");
                let to_val = obj.get("to").and_then(|v| v.as_str()).unwrap_or("0");
                lines.push(format!("{}={}||{}||{}||{}||Y", key, obj.get("value").and_then(|v| v.as_str()).unwrap_or("0"), from_val, step, to_val));
            } else {
                lines.push(format!("{}={}", key, obj.get("value").and_then(|v| v.as_str()).unwrap_or("0")));
            }
        }
    }

    fs::write(path, lines.join("\n"))?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "path": path,
            "parameters_written": lines.len(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_patch_set_file(args: &Value) -> Result<Value> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;

    let patches = args.get("patches")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("patches object is required"))?;

    let content = fs::read_to_string(path)?;
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut patched_count = 0;

    for (key, value) in patches {
        let new_value = if let Some(s) = value.as_str() {
            s.to_string()
        } else if let Some(n) = value.as_f64() {
            n.to_string()
        } else if let Some(b) = value.as_bool() {
            if b { "true".to_string() } else { "false".to_string() }
        } else {
            value.to_string()
        };

        let mut found = false;
        for line in &mut lines {
            if line.starts_with(&format!("{}:", key)) {
                *line = format!("{}: {}", key, new_value);
                found = true;
                patched_count += 1;
                break;
            } else if line.starts_with(&format!("{}=", key)) {
                *line = format!("{}={}", key, new_value);
                found = true;
                patched_count += 1;
                break;
            }
        }

        if !found {
            lines.push(format!("{}: {}", key, new_value));
            patched_count += 1;
        }
    }

    fs::write(path, lines.join("\n"))?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "path": path,
            "parameters_patched": patched_count,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_clone_set_file(args: &Value) -> Result<Value> {
    let source = args.get("source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("source is required"))?;

    let destination = args.get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("destination is required"))?;

    fs::copy(source, destination)?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "source": source,
            "destination": destination,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_diff_set_files(args: &Value) -> Result<Value> {
    let file_a = args.get("file_a")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("file_a is required"))?;

    let file_b = args.get("file_b")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("file_b is required"))?;

    let content_a = fs::read_to_string(file_a)?;
    let content_b = fs::read_to_string(file_b)?;

    let mut differences = Vec::new();

    for (i, (line_a, line_b)) in content_a.lines().zip(content_b.lines()).enumerate() {
        if line_a != line_b {
            differences.push(json!({
                "line": i + 1,
                "file_a": line_a,
                "file_b": line_b,
            }));
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "file_a": file_a,
            "file_b": file_b,
            "differences": differences,
            "total_differences": differences.len(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_set_from_optimization(args: &Value) -> Result<Value> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;

    let params = args.get("params")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("params is required"))?;

    let mut lines = Vec::new();
    for (key, value) in params {
        if let Some(val_str) = value.as_str() {
            lines.push(format!("{}={}", key, val_str));
        }
    }

    fs::write(path, lines.join("\n"))?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "path": path,
            "parameters_written": lines.len(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_describe_sweep(args: &Value) -> Result<Value> {
    let path = args.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;

    let content = fs::read_to_string(path)?;
    let mut sweep_params = serde_json::Map::new();

    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            
            if value.contains("||Y") {
                if let Some((from_val, to_val)) = value.split_once("..") {
                    sweep_params.insert(key.to_string(), json!({
                        "from": from_val.trim(),
                        "to": to_val.trim().replace("||Y", ""),
                        "step": 1.0
                    }));
                }
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "path": path,
            "sweep_params": sweep_params
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_list_set_files(config: &Config) -> Result<Value> {
    let mut set_files = Vec::new();

    if let Some(tester_dir) = &config.tester_profiles_dir {
        if let Ok(entries) = fs::read_dir(tester_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "set").unwrap_or(false) {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let content = fs::read_to_string(&path).unwrap_or_default();
                    let param_count = content.lines().filter(|l| l.contains(':')).count();
                    let sweep_count = content.lines().filter(|l| l.contains("||Y")).count();

                    set_files.push(json!({
                        "name": name,
                        "path": path.to_string_lossy(),
                        "param_count": param_count,
                        "sweep_count": sweep_count
                    }));
                }
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({ "set_files": set_files }).to_string() }],
        "isError": false
    }))
}
