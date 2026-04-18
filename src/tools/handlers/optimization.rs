use anyhow::Result;
use serde_json::{json, Value};
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
        model: 0,
        leverage: args.get("leverage").and_then(|v| v.as_u64()).unwrap_or(500) as u32,
        currency: args.get("currency").and_then(|v| v.as_str()).unwrap_or("USD").to_string(),
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

    Ok(json!({
        "content": [{ "type": "text", "text": status.to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_optimization_results(_config: &Config, args: &Value) -> Result<Value> {
    let job_id = args.get("job_id")
        .and_then(|v| v.as_str());

    let file = args.get("file")
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
    let top_n = args.get("top").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

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
