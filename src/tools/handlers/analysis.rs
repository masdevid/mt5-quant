use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use crate::analytics::DealAnalyzer;
use crate::models::deals::Deal;
use crate::models::metrics::Metrics;

/// Helper to load deals and metrics from report directory
fn load_report_data(report_dir: &str) -> Result<(Vec<Deal>, Metrics)> {
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

    Ok((deals, metrics))
}

pub async fn handle_analyze_report(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, metrics) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();

    // Check if specific analytics requested
    let requested: Option<HashSet<String>> = args.get("analytics")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect());

    let top_losses_limit = args.get("top_losses_limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let all = requested.is_none();
    let req = |name: &str| all || requested.as_ref().map(|s| s.contains(name)).unwrap_or(false);

    // Build selective result
    let mut result = json!({});

    if req("monthly_pnl") || all {
        result["monthly"] = json!(analyzer.monthly_pnl(&deals));
    }
    if req("drawdown_events") || all {
        result["dd_events"] = json!(analyzer.reconstruct_dd_events(&deals, &metrics));
    }
    if req("top_losses") || all {
        result["top_losses"] = json!(analyzer.top_losses(&deals, top_losses_limit));
    }
    if req("loss_sequences") || all {
        result["loss_sequences"] = json!(analyzer.loss_sequences(&deals));
    }
    if req("position_pairs") || all {
        result["position_pairs"] = json!(analyzer.position_pairs(&deals));
    }
    if req("direction_bias") || all {
        result["direction_bias"] = json!(analyzer.direction_bias(&deals));
    }
    if req("streak_analysis") || all {
        result["streak_analysis"] = json!(analyzer.streak_analysis(&deals));
    }
    if req("concurrent_peak") || all {
        result["concurrent_peak"] = json!(analyzer.concurrent_peak(&deals));
    }

    let analysis_path = Path::new(report_dir).join("analysis.json");
    fs::write(&analysis_path, serde_json::to_string_pretty(&result)?)?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "analysis_file": analysis_path.to_string_lossy(),
            "analytics_run": requested.map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_else(|| vec!["all".to_string()]),
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

// === Granular Analytics Handlers ===

pub async fn handle_analyze_monthly_pnl(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.monthly_pnl(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "monthly_pnl": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_drawdown_events(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, metrics) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.reconstruct_dd_events(&deals, &metrics);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "drawdown_events": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_top_losses(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.top_losses(&deals, limit);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "limit": limit,
            "top_losses": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_loss_sequences(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.loss_sequences(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "loss_sequences": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_position_pairs(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.position_pairs(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "position_pairs": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_direction_bias(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.direction_bias(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "direction_bias": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_streaks(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.streak_analysis(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "streak_analysis": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_concurrent_peak(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.concurrent_peak(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "concurrent_peak": result,
        }).to_string() }],
        "isError": false
    }))
}

// === Deal Query Handlers ===

pub async fn handle_list_deals(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;

    // Apply filters
    let deal_type = args.get("deal_type").and_then(|v| v.as_str());
    let min_profit = args.get("min_profit").and_then(|v| v.as_f64());
    let max_profit = args.get("max_profit").and_then(|v| v.as_f64());
    let start_date = args.get("start_date").and_then(|v| v.as_str());
    let end_date = args.get("end_date").and_then(|v| v.as_str());
    let min_volume = args.get("min_volume").and_then(|v| v.as_f64());
    let max_volume = args.get("max_volume").and_then(|v| v.as_f64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let mut filtered: Vec<&Deal> = deals.iter().filter(|d| {
        // Only include closed trades with non-zero profit
        if !d.entry.to_lowercase().contains("out") || d.profit == 0.0 {
            return false;
        }

        if let Some(dt) = deal_type {
            if !d.deal_type.to_lowercase().contains(dt) {
                return false;
            }
        }

        if let Some(min) = min_profit {
            if d.profit < min {
                return false;
            }
        }

        if let Some(max) = max_profit {
            if d.profit > max {
                return false;
            }
        }

        if let Some(start) = start_date {
            if !d.time.starts_with(start) && d.time < start.to_string() {
                return false;
            }
        }

        if let Some(end) = end_date {
            if d.time > end.to_string() {
                return false;
            }
        }

        if let Some(min) = min_volume {
            if d.volume < min {
                return false;
            }
        }

        if let Some(max) = max_volume {
            if d.volume > max {
                return false;
            }
        }

        true
    }).collect();

    // Sort by time descending
    filtered.sort_by(|a, b| b.time.cmp(&a.time));
    filtered.truncate(limit);

    let deal_list: Vec<Value> = filtered
        .iter()
        .map(|d| json!({
            "time": d.time,
            "deal": d.deal,
            "symbol": d.symbol,
            "deal_type": d.deal_type,
            "volume": d.volume,
            "price": d.price,
            "profit": d.profit,
            "commission": d.commission,
            "swap": d.swap,
            "comment": d.comment,
            "magic": d.magic,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "total_deals": deals.len(),
            "filtered_count": deal_list.len(),
            "deals": deal_list,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_deals_by_comment(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("query is required"))?;

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let (deals, _) = load_report_data(report_dir)?;

    let query_lower = query.to_lowercase();
    let mut filtered: Vec<&Deal> = deals
        .iter()
        .filter(|d| {
            d.entry.to_lowercase().contains("out")
                && d.profit != 0.0
                && d.comment.to_lowercase().contains(&query_lower)
        })
        .collect();

    filtered.sort_by(|a, b| b.time.cmp(&a.time));
    filtered.truncate(limit);

    let deal_list: Vec<Value> = filtered
        .iter()
        .map(|d| json!({
            "time": d.time,
            "deal": d.deal,
            "symbol": d.symbol,
            "deal_type": d.deal_type,
            "volume": d.volume,
            "profit": d.profit,
            "comment": d.comment,
            "magic": d.magic,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "query": query,
            "matched": deal_list.len(),
            "deals": deal_list,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_deals_by_magic(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let magic = args.get("magic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("magic is required"))?;

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let (deals, _) = load_report_data(report_dir)?;

    let mut filtered: Vec<&Deal> = deals
        .iter()
        .filter(|d| {
            d.entry.to_lowercase().contains("out")
                && d.profit != 0.0
                && d.magic.as_ref().map(|m| m.contains(magic)).unwrap_or(false)
        })
        .collect();

    filtered.sort_by(|a, b| b.time.cmp(&a.time));
    filtered.truncate(limit);

    let deal_list: Vec<Value> = filtered
        .iter()
        .map(|d| json!({
            "time": d.time,
            "deal": d.deal,
            "symbol": d.symbol,
            "deal_type": d.deal_type,
            "volume": d.volume,
            "profit": d.profit,
            "comment": d.comment,
            "magic": d.magic,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "magic": magic,
            "matched": deal_list.len(),
            "deals": deal_list,
        }).to_string() }],
        "isError": false
    }))
}

// === New Analytics Handlers ===

pub async fn handle_analyze_profit_distribution(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.profit_distribution(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "profit_distribution": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_time_performance(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.time_performance(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "time_performance": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_hold_time_distribution(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.hold_time_analysis(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "hold_time_analysis": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_layer_performance(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.layer_performance(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "layer_performance": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_volume_vs_profit(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.volume_analysis(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "volume_analysis": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_costs(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, _) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.cost_analysis(&deals);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "cost_analysis": result,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_analyze_efficiency(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let (deals, metrics) = load_report_data(report_dir)?;
    let analyzer = DealAnalyzer::new();
    let result = analyzer.efficiency_analysis(&deals, &metrics);

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "efficiency_analysis": result,
        }).to_string() }],
        "isError": false
    }))
}

// Import Config for analysis module
use crate::models::Config;
