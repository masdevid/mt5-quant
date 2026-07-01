use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use crate::models::Config;
use crate::optimization::{OptimizationParams, OptimizationParser, OptimizationRunner};

pub async fn handle_run_optimization(config: &Config, args: &Value) -> Result<Value> {
    let expert = args.get("expert")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("expert is required"))?;

    let set_file = args.get("set_file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("set_file is required"))?;

    let from_date = args.get("from_date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("from_date is required"))?;

    let to_date = args.get("to_date")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("to_date is required"))?;

    let params = OptimizationParams {
        expert: expert.to_string(),
        set_file: set_file.to_string(),
        symbol: args.get("symbol").and_then(|v| v.as_str()).unwrap_or("XAUUSD").to_string(),
        from_date: from_date.to_string(),
        to_date: to_date.to_string(),
        deposit: args.get("deposit").and_then(|v| v.as_u64()).unwrap_or(10000) as u32,
        leverage: args.get("leverage").and_then(|v| v.as_u64()).unwrap_or(500) as u32,
        currency: args.get("currency").and_then(|v| v.as_str()).unwrap_or("USD").to_string(),
        max_passes: args.get("max_passes").and_then(|v| v.as_u64()).map(|v| v as u32),
    };

    let runner = OptimizationRunner::new(config.clone());
    let result = runner.run(params).await?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": result.success,
            "job_id": result.job_id,
            "pid": result.pid,
            "log_file": result.log_file.to_string_lossy(),
            "combinations": result.combinations,
            "message": result.message,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_optimization_status(config: &Config, args: &Value) -> Result<Value> {
    let job_id = args.get("job_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("job_id is required"))?;

    let runner = OptimizationRunner::new(config.clone());
    let status = runner.get_job_status(job_id)?;

    // Store completion results back to metadata for persistence
    if status.get("status").and_then(|v| v.as_str()) == Some("completed") {
        let jobs_dir = Path::new(".mt5mcp_jobs");
        let meta_path = jobs_dir.join(format!("{}.json", job_id));
        if let Ok(meta_str) = fs::read_to_string(&meta_path) {
            if let Ok(mut meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert("status".into(), serde_json::Value::String("completed".into()));
                    obj.insert("completed_at".into(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));
                    if let Some(top) = status.get("top_10") {
                        obj.insert("top_10".into(), top.clone());
                    }
                    if let Some(best) = status.get("best_pf") {
                        obj.insert("best_pf".into(), best.clone());
                    }
                    if let Some(total) = status.get("total_passes") {
                        obj.insert("total_passes".into(), total.clone());
                    }
                    let _ = fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap_or_default());
                }
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": status.to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_optimization_results(_config: &Config, args: &Value) -> Result<Value> {
    let job_id = args.get("job_id")
        .and_then(|v| v.as_str());

    let file = args.get("report_file")
        .and_then(|v| v.as_str());

    let parser = OptimizationParser::new();
    
    let passes = if let Some(jid) = job_id {
        parser.parse_job(jid)?
    } else if let Some(f) = file {
        parser.parse_file(std::path::Path::new(f))?
    } else {
        return Err(anyhow::anyhow!("Either job_id or file is required"));
    };

    let sort_by = args.get("sort").and_then(|v| v.as_str()).unwrap_or("profit");
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

    let best = parser.find_best_pass(&passes, sort_by);

    let mut sorted_passes = passes.clone();
    sorted_passes.sort_by(|a, b| b.profit.partial_cmp(&a.profit).unwrap());
    sorted_passes.truncate(top_n);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "total_passes": passes.len(),
            "top_passes": sorted_passes,
            "best": best,
            "sort_by": sort_by,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_list_jobs(config: &Config) -> Result<Value> {
    let runner = OptimizationRunner::new(config.clone());
    let jobs = runner.list_jobs()?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({ "jobs": jobs }).to_string() }],
        "isError": false
    }))
}
