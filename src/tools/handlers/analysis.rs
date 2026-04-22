use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use crate::analytics::DealAnalyzer;
use crate::models::deals::Deal;
use crate::models::metrics::Metrics;
use crate::models::Config;
use crate::storage::ReportDb;

// ── Internal helpers ──────────────────────────────────────────────────────────

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} is required", key))
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

/// Resolve a report from args (report_id > report_dir > latest).
/// Returns (deals, metrics, report_dir).
fn resolve_report(args: &Value) -> Result<(Vec<Deal>, Metrics, String)> {
    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let entry = if let Some(id) = args.get("report_id").and_then(|v| v.as_str()) {
        db.get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Report '{}' not found in DB", id))?
    } else if let Some(dir) = args.get("report_dir").and_then(|v| v.as_str()) {
        db.get_by_report_dir(dir)?
            .ok_or_else(|| anyhow::anyhow!(
                "No DB entry for report_dir '{}'. This report may predate DB storage.", dir
            ))?
    } else {
        db.get_latest()?
            .ok_or_else(|| anyhow::anyhow!("No reports in DB. Run a backtest first."))?
    };

    let deals = db.get_deals(&entry.id)?;

    let metrics_path = Path::new(&entry.report_dir).join("metrics.json");
    let metrics = if metrics_path.exists() {
        serde_json::from_str(&fs::read_to_string(&metrics_path)?)?
    } else {
        Metrics::default()
    };

    Ok((deals, metrics, entry.report_dir))
}

fn prepare_analysis(args: &Value) -> Result<(Vec<Deal>, Metrics, DealAnalyzer, String)> {
    let (deals, metrics, report_dir) = resolve_report(args)?;
    Ok((deals, metrics, DealAnalyzer::new(), report_dir))
}

// ── Composite analytics ───────────────────────────────────────────────────────

pub async fn handle_analyze_report(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, metrics, analyzer, report_dir) = prepare_analysis(args)?;

    let requested: Option<HashSet<String>> = args.get("analytics")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());

    let top_losses_limit = args.get("top_losses_limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let all = requested.is_none();
    let req = |name: &str| all || requested.as_ref().map(|s| s.contains(name)).unwrap_or(false);

    let mut result = json!({});

    if req("monthly_pnl")      { result["monthly"]          = json!(analyzer.monthly_pnl(&deals)); }
    if req("drawdown_events")  { result["dd_events"]        = json!(analyzer.reconstruct_dd_events(&deals, &metrics)); }
    if req("top_losses")       { result["top_losses"]       = json!(analyzer.top_losses(&deals, top_losses_limit)); }
    if req("loss_sequences")   { result["loss_sequences"]   = json!(analyzer.loss_sequences(&deals)); }
    if req("position_pairs")   { result["position_pairs"]   = json!(analyzer.position_pairs(&deals)); }
    if req("direction_bias")   { result["direction_bias"]   = json!(analyzer.direction_bias(&deals)); }
    if req("streak_analysis")  { result["streak_analysis"]  = json!(analyzer.streak_analysis(&deals)); }
    if req("concurrent_peak")  { result["concurrent_peak"]  = json!(analyzer.concurrent_peak(&deals)); }

    let analysis_path = Path::new(&report_dir).join("analysis.json");
    fs::write(&analysis_path, serde_json::to_string_pretty(&result)?)?;

    Ok(ok_response(json!({
        "success": true,
        "analysis_file": analysis_path.to_string_lossy(),
        "analytics_run": requested.map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_else(|| vec!["all".to_string()]),
        "summary": result,
    })))
}

pub async fn handle_compare_baseline(_config: &Config, args: &Value) -> Result<Value> {
    let (_, _, _, report_dir) = prepare_analysis(args)?;

    let baseline_path = Path::new("config/baseline.json");
    let metrics_path = Path::new(&report_dir).join("metrics.json");

    if !baseline_path.exists() {
        return Ok(ok_response(json!("No baseline.json found in config/")));
    }

    let baseline: Value = serde_json::from_str(&fs::read_to_string(baseline_path)?)?;
    let current: Value = serde_json::from_str(&fs::read_to_string(metrics_path)?)?;

    Ok(ok_response(json!({
        "baseline": baseline,
        "current": current,
        "improvements": {
            "profit": current.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0)
                - baseline.get("net_profit").and_then(|v| v.as_f64()).unwrap_or(0.0),
            "drawdown": current.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0)
                - baseline.get("max_dd_pct").and_then(|v| v.as_f64()).unwrap_or(0.0),
        }
    })))
}

// ── Granular analytics handlers ───────────────────────────────────────────────

pub async fn handle_analyze_monthly_pnl(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "monthly_pnl": analyzer.monthly_pnl(&deals) })))
}

pub async fn handle_analyze_drawdown_events(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, metrics, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "drawdown_events": analyzer.reconstruct_dd_events(&deals, &metrics) })))
}

pub async fn handle_analyze_top_losses(_config: &Config, args: &Value) -> Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "limit": limit, "top_losses": analyzer.top_losses(&deals, limit) })))
}

pub async fn handle_analyze_loss_sequences(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "loss_sequences": analyzer.loss_sequences(&deals) })))
}

pub async fn handle_analyze_position_pairs(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "position_pairs": analyzer.position_pairs(&deals) })))
}

pub async fn handle_analyze_direction_bias(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "direction_bias": analyzer.direction_bias(&deals) })))
}

pub async fn handle_analyze_streaks(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "streak_analysis": analyzer.streak_analysis(&deals) })))
}

pub async fn handle_analyze_concurrent_peak(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "concurrent_peak": analyzer.concurrent_peak(&deals) })))
}

pub async fn handle_analyze_profit_distribution(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "profit_distribution": analyzer.profit_distribution(&deals) })))
}

pub async fn handle_analyze_time_performance(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "time_performance": analyzer.time_performance(&deals) })))
}

pub async fn handle_analyze_hold_time_distribution(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "hold_time_analysis": analyzer.hold_time_analysis(&deals) })))
}

pub async fn handle_analyze_layer_performance(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "layer_performance": analyzer.layer_performance(&deals) })))
}

pub async fn handle_analyze_volume_vs_profit(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "volume_analysis": analyzer.volume_analysis(&deals) })))
}

pub async fn handle_analyze_costs(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "cost_analysis": analyzer.cost_analysis(&deals) })))
}

pub async fn handle_analyze_efficiency(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, metrics, analyzer, _) = prepare_analysis(args)?;
    Ok(ok_response(json!({ "success": true, "efficiency_analysis": analyzer.efficiency_analysis(&deals, &metrics) })))
}

// ── Deal query handlers ───────────────────────────────────────────────────────

fn deal_to_json(d: &Deal) -> Value {
    json!({
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
    })
}

fn is_closed_trade(d: &Deal) -> bool {
    d.entry.to_lowercase().contains("out") && d.profit != 0.0
}

pub async fn handle_list_deals(_config: &Config, args: &Value) -> Result<Value> {
    let (deals, _, _, _) = prepare_analysis(args)?;

    let deal_type  = args.get("deal_type").and_then(|v| v.as_str());
    let min_profit = args.get("min_profit").and_then(|v| v.as_f64());
    let max_profit = args.get("max_profit").and_then(|v| v.as_f64());
    let start_date = args.get("start_date").and_then(|v| v.as_str());
    let end_date   = args.get("end_date").and_then(|v| v.as_str());
    let min_volume = args.get("min_volume").and_then(|v| v.as_f64());
    let max_volume = args.get("max_volume").and_then(|v| v.as_f64());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let mut filtered: Vec<&Deal> = deals.iter().filter(|d| {
        if !is_closed_trade(d) { return false; }
        if let Some(dt)  = deal_type  { if !d.deal_type.to_lowercase().contains(dt)           { return false; } }
        if let Some(min) = min_profit { if d.profit < min                                      { return false; } }
        if let Some(max) = max_profit { if d.profit > max                                      { return false; } }
        if let Some(s)   = start_date { if d.time.as_str() < s                                 { return false; } }
        if let Some(e)   = end_date   { if d.time.as_str() > e                                 { return false; } }
        if let Some(min) = min_volume { if d.volume < min                                      { return false; } }
        if let Some(max) = max_volume { if d.volume > max                                      { return false; } }
        true
    }).collect();

    filtered.sort_by(|a, b| b.time.cmp(&a.time));
    filtered.truncate(limit);

    Ok(ok_response(json!({
        "success": true,
        "total_deals": deals.len(),
        "filtered_count": filtered.len(),
        "deals": filtered.iter().map(|d| deal_to_json(d)).collect::<Vec<_>>(),
    })))
}

pub async fn handle_search_deals_by_comment(_config: &Config, args: &Value) -> Result<Value> {
    let query = required_str(args, "query")?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let (deals, _, _, _) = prepare_analysis(args)?;
    let query_lower = query.to_lowercase();

    let mut filtered: Vec<&Deal> = deals.iter()
        .filter(|d| is_closed_trade(d) && d.comment.to_lowercase().contains(&query_lower))
        .collect();

    filtered.sort_by(|a, b| b.time.cmp(&a.time));
    filtered.truncate(limit);

    Ok(ok_response(json!({
        "success": true,
        "query": query,
        "matched": filtered.len(),
        "deals": filtered.iter().map(|d| deal_to_json(d)).collect::<Vec<_>>(),
    })))
}

pub async fn handle_search_deals_by_magic(_config: &Config, args: &Value) -> Result<Value> {
    let magic = required_str(args, "magic")?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let (deals, _, _, _) = prepare_analysis(args)?;

    let mut filtered: Vec<&Deal> = deals.iter()
        .filter(|d| is_closed_trade(d) && d.magic.as_ref().map(|m| m.contains(magic)).unwrap_or(false))
        .collect();

    filtered.sort_by(|a, b| b.time.cmp(&a.time));
    filtered.truncate(limit);

    Ok(ok_response(json!({
        "success": true,
        "magic": magic,
        "matched": filtered.len(),
        "deals": filtered.iter().map(|d| deal_to_json(d)).collect::<Vec<_>>(),
    })))
}

#[allow(dead_code)]
fn _err_response_available() { let _ = err_response(""); }
