use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use crate::analytics::DealAnalyzer;
use crate::models::deals::Deal;
use crate::models::metrics::Metrics;

pub async fn handle_analyze_report(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let deals_csv = Path::new(report_dir).join("deals.csv");
    let metrics_json = Path::new(report_dir).join("metrics.json");

    if !deals_csv.exists() {
        return Err(anyhow::anyhow!("deals.csv not found in {}", report_dir));
    }

    let deals = read_deals_from_csv(&deals_csv)?;
    
    let metrics = if metrics_json.exists() {
        let content = fs::read_to_string(&metrics_json)?;
        serde_json::from_str(&content)?
    } else {
        Metrics::default()
    };

    let _strategy = args.get("strategy").and_then(|v| v.as_str()).unwrap_or("grid");
    let _deep = args.get("deep").and_then(|v| v.as_bool()).unwrap_or(false);

    let analyzer = DealAnalyzer::new();
    let result = analyzer.analyze(&deals, &metrics);

    let analysis_path = Path::new(report_dir).join("analysis.json");
    fs::write(&analysis_path, serde_json::to_string_pretty(&result)?)?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "analysis_file": analysis_path.to_string_lossy(),
            "summary": result,
        }).to_string() }],
        "isError": false
    }))
}

fn read_deals_from_csv(path: &Path) -> Result<Vec<Deal>> {
    let content = fs::read_to_string(path)?;
    let mut deals = Vec::new();
    
    let mut lines = content.lines();
    let _header = lines.next();
    
    for line in lines {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 12 {
            deals.push(Deal {
                time: parts[0].to_string(),
                deal: parts[1].to_string(),
                symbol: parts[2].to_string(),
                deal_type: parts[3].to_string(),
                entry: parts[4].to_string(),
                volume: parts[5].parse().unwrap_or(0.0),
                price: parts[6].parse().unwrap_or(0.0),
                order: parts[7].to_string(),
                commission: parts[8].parse().unwrap_or(0.0),
                swap: parts[9].parse().unwrap_or(0.0),
                profit: parts[10].parse().unwrap_or(0.0),
                balance: parts[11].parse().unwrap_or(0.0),
                comment: parts.get(12).unwrap_or(&"").to_string(),
                magic: parts.get(13).map(|s| s.to_string()),
            });
        }
    }
    
    Ok(deals)
}

pub async fn handle_compare_baseline(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let baseline_path = Path::new("config/baseline.json");
    let metrics_path = Path::new(report_dir).join("metrics.json");

    if !baseline_path.exists() {
        return Ok(json!({
            "content": [{ "type": "text", "text": "No baseline.json found in config/" }],
            "isError": false
        }));
    }

    let baseline: Value = serde_json::from_str(&fs::read_to_string(baseline_path)?)?;
    let current: Value = serde_json::from_str(&fs::read_to_string(metrics_path)?)?;

    let comparison = json!({
        "baseline": baseline,
        "current": current,
        "improvements": {
            "profit": current.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0) 
                - baseline.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0),
            "drawdown": current.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0)
                - baseline.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0),
        }
    });

    Ok(json!({
        "content": [{ "type": "text", "text": comparison.to_string() }],
        "isError": false
    }))
}

// Import Config for analysis module
use crate::models::Config;
