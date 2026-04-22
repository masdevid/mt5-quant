use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use crate::analytics::ReportExtractor;
use crate::models::Config;
use crate::storage::{ReportDb, ReportFilters};

pub async fn handle_get_latest_report(_config: &Config, args: &Value) -> Result<Value> {
    let include_chart = args.get("include_chart").and_then(|v| v.as_bool()).unwrap_or(true);

    let db = ReportDb::new(&Config::db_path());
    if let Err(e) = db.init() {
        return Ok(json!({
            "content": [{ "type": "text", "text": format!("DB error: {}", e) }],
            "isError": true
        }));
    }

    match db.get_latest()? {
        Some(entry) => {
            let mut response = json!({
                "success": true,
                "report": {
                    "id": entry.id,
                    "expert": entry.expert,
                    "symbol": entry.symbol,
                    "timeframe": entry.timeframe,
                    "from_date": entry.from_date,
                    "to_date": entry.to_date,
                    "created_at": entry.created_at,
                    "net_profit": entry.net_profit,
                    "profit_factor": entry.profit_factor,
                    "max_dd_pct": entry.max_dd_pct,
                    "sharpe_ratio": entry.sharpe_ratio,
                    "total_trades": entry.total_trades,
                    "win_rate_pct": entry.win_rate_pct,
                    "recovery_factor": entry.recovery_factor,
                    "deposit": entry.deposit,
                    "currency": entry.currency,
                    "leverage": entry.leverage,
                    "duration_seconds": entry.duration_seconds,
                    "set_file_original": entry.set_file_original,
                    "set_snapshot_path": entry.set_snapshot_path,
                    "report_dir": entry.report_dir,
                    "charts_dir": entry.charts_dir,
                    "tags": entry.tags,
                    "notes": entry.notes,
                    "verdict": entry.verdict,
                }
            });

            // Include equity chart as base64 if requested and available
            if include_chart {
                if let Some(charts_dir) = &entry.charts_dir {
                    let chart_path = Path::new(charts_dir).join("equity.png");
                    if chart_path.exists() {
                        match fs::read(&chart_path) {
                            Ok(bytes) => {
                                let base64 = BASE64.encode(&bytes);
                                response["report"]["equity_chart_base64"] = json!(base64);
                                response["report"]["equity_chart_format"] = json!("png");
                            }
                            Err(e) => {
                                response["report"]["equity_chart_error"] = json!(format!("Failed to read chart: {}", e));
                            }
                        }
                    } else {
                        response["report"]["equity_chart_error"] = json!("equity.png not found in charts_dir");
                    }
                } else {
                    response["report"]["equity_chart_error"] = json!("No charts_dir available for this report");
                }
            }

            Ok(json!({
                "content": [{ "type": "text", "text": response.to_string() }],
                "isError": false
            }))
        }
        None => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "error": "No reports found in database"
                }).to_string() }],
                "isError": true
            }))
        }
    }
}

pub async fn handle_list_reports(args: &Value) -> Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

    let db = ReportDb::new(&Config::db_path());
    if let Err(e) = db.init() {
        return Ok(json!({
            "content": [{ "type": "text", "text": format!("DB error: {}", e) }],
            "isError": true
        }));
    }

    let filters = ReportFilters::default();
    let entries = db.list(limit, &filters)?;
    let total = db.count().unwrap_or(0);

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "charts_dir": e.charts_dir,
            "report_dir": e.report_dir,
            "verdict": e.verdict,
            "tags": e.tags,
            "notes": e.notes,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "total": total,
            "returned": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_reports(args: &Value) -> Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let filters = ReportFilters {
        expert: args.get("expert").and_then(|v| v.as_str()).map(|s| s.to_string()),
        symbol: args.get("symbol").and_then(|v| v.as_str()).map(|s| s.to_string()),
        timeframe: args.get("timeframe").and_then(|v| v.as_str()).map(|s| s.to_string()),
        created_after: args.get("after").and_then(|v| v.as_str()).map(|s| s.to_string()),
        min_profit: args.get("min_profit").and_then(|v| v.as_f64()),
        max_dd: args.get("max_dd").and_then(|v| v.as_f64()),
        verdict: args.get("verdict").and_then(|v| v.as_str()).map(|s| s.to_string()),
    };

    let entries = db.list(limit, &filters)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "set_snapshot": e.set_snapshot_path,
            "charts_dir": e.charts_dir,
            "report_dir": e.report_dir,
            "verdict": e.verdict,
            "tags": e.tags,
            "notes": e.notes,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_prune_reports(_config: &Config, args: &Value) -> Result<Value> {
    let keep_last = args.get("keep_last").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let purgeable = db.list_purgeable(keep_last)?;
    let mut pruned = 0;
    let mut freed_bytes: u64 = 0;

    for (id, report_dir, charts_dir) in &purgeable {
        if !dry_run {
            freed_bytes += super::dir_size(Path::new(report_dir));
            let _ = fs::remove_dir_all(report_dir);

            if let Some(cd) = charts_dir {
                freed_bytes += super::dir_size(Path::new(cd));
                let _ = fs::remove_dir_all(cd);
            }

            let _ = db.delete_entry(id);
            pruned += 1;
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "pruned": pruned,
            "would_prune": purgeable.len(),
            "kept": keep_last,
            "freed_bytes": freed_bytes,
            "dry_run": dry_run,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_tail_log(_config: &Config, args: &Value) -> Result<Value> {
    let job_id = args.get("job_id").and_then(|v| v.as_str());

    let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let log_path = if let Some(jid) = job_id {
        let jobs_dir = Path::new(".mt5mcp_jobs");
        let meta_path = jobs_dir.join(format!("{}.json", jid));
        let meta: Value = serde_json::from_str(&fs::read_to_string(meta_path)?)?;
        meta.get("log_file").and_then(|v| v.as_str()).map(|s| s.to_string())
    } else {
        args.get("file").and_then(|v| v.as_str()).map(|s| s.to_string())
    };

    let log_path = log_path.ok_or_else(|| anyhow::anyhow!("Could not determine log file"))?;

    let content = fs::read_to_string(&log_path)?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines);
    let last_lines = &all_lines[start..];

    Ok(json!({
        "content": [{ "type": "text", "text": last_lines.join("\n") }],
        "isError": false
    }))
}

pub async fn handle_archive_report(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let delete_after = args.get("delete_after").and_then(|v| v.as_bool()).unwrap_or(false);

    let history_dir = Path::new(".mt5mcp_history");
    fs::create_dir_all(history_dir)?;

    let report_name = Path::new(report_dir).file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let archive_path = history_dir.join(format!("{}.tar.gz", report_name));

    let status = std::process::Command::new("tar")
        .args(["-czf", &archive_path.to_string_lossy(), "-C", 
               Path::new(report_dir).parent().unwrap().to_str().unwrap(), report_name])
        .status()?;

    if delete_after && status.success() {
        fs::remove_dir_all(report_dir)?;
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": status.success(),
            "archive_path": archive_path.to_string_lossy(),
            "deleted": delete_after && status.success(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_archive_all_reports(config: &Config, args: &Value) -> Result<Value> {
    let keep_last = args.get("keep_last").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let reports_dir = config.reports_dir();
    let history_dir = Path::new(".mt5mcp_history");
    fs::create_dir_all(history_dir)?;

    let mut archived = 0;

    if let Ok(entries) = fs::read_dir(&reports_dir) {
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by(|a, b| {
            b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
                .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH))
        });

        for entry in entries.into_iter().skip(keep_last) {
            let path = entry.path();
            if path.is_dir() && !path.to_string_lossy().ends_with("_opt") {
                let report_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
                let archive_path = history_dir.join(format!("{}.tar.gz", report_name));

                let _ = std::process::Command::new("tar")
                    .args(["-czf", &archive_path.to_string_lossy(), "-C", 
                           path.parent().unwrap().to_str().unwrap(), report_name])
                    .status();

                let _ = fs::remove_dir_all(&path);
                archived += 1;
            }
        }
    }

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "archived": archived,
            "kept": keep_last,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_promote_to_baseline(_config: &Config, args: &Value) -> Result<Value> {
    let report_dir = args.get("report_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("report_dir is required"))?;

    let metrics_path = Path::new(report_dir).join("metrics.json");
    let baseline_path = Path::new("config/baseline.json");

    fs::copy(&metrics_path, &baseline_path)?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "baseline_file": baseline_path.to_string_lossy(),
            "source": metrics_path.to_string_lossy(),
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_history(args: &Value) -> Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let filters = ReportFilters {
        expert: args.get("ea").and_then(|v| v.as_str()).map(|s| s.to_string()),
        symbol: args.get("symbol").and_then(|v| v.as_str()).map(|s| s.to_string()),
        verdict: args.get("verdict").and_then(|v| v.as_str()).map(|s| s.to_string()),
        ..Default::default()
    };

    let entries = db.list(limit, &filters)?;
    let total = db.count().unwrap_or(0);

    let history: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "set_file": e.set_file_original,
            "set_snapshot": e.set_snapshot_path,
            "charts_dir": e.charts_dir,
            "verdict": e.verdict,
            "tags": e.tags,
            "notes": e.notes,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "total": total,
            "returned": history.len(),
            "history": history,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_annotate_history(args: &Value) -> Result<Value> {
    let report_id = args
        .get("history_id")
        .or_else(|| args.get("report_name"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("history_id is required"))?;

    let notes = args.get("notes").and_then(|v| v.as_str());
    let verdict = args.get("verdict").and_then(|v| v.as_str());
    let tags: Option<Vec<String>> = args
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let updated = db.annotate(report_id, notes, tags, verdict)?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": updated,
            "id": report_id,
            "notes": notes,
            "verdict": verdict,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_report_by_id(_config: &Config, args: &Value) -> Result<Value> {
    let id = args.get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("id is required"))?;
    let include_chart = args.get("include_chart").and_then(|v| v.as_bool()).unwrap_or(true);

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    match db.get_by_id(id)? {
        Some(entry) => {
            let mut response = json!({
                "success": true,
                "report": {
                    "id": entry.id,
                    "expert": entry.expert,
                    "symbol": entry.symbol,
                    "timeframe": entry.timeframe,
                    "from_date": entry.from_date,
                    "to_date": entry.to_date,
                    "created_at": entry.created_at,
                    "net_profit": entry.net_profit,
                    "profit_factor": entry.profit_factor,
                    "max_dd_pct": entry.max_dd_pct,
                    "sharpe_ratio": entry.sharpe_ratio,
                    "total_trades": entry.total_trades,
                    "win_rate_pct": entry.win_rate_pct,
                    "recovery_factor": entry.recovery_factor,
                    "deposit": entry.deposit,
                    "currency": entry.currency,
                    "leverage": entry.leverage,
                    "duration_seconds": entry.duration_seconds,
                    "set_file_original": entry.set_file_original,
                    "set_snapshot_path": entry.set_snapshot_path,
                    "report_dir": entry.report_dir,
                    "charts_dir": entry.charts_dir,
                    "tags": entry.tags,
                    "notes": entry.notes,
                    "verdict": entry.verdict,
                }
            });

            if include_chart {
                if let Some(charts_dir) = &entry.charts_dir {
                    let chart_path = Path::new(charts_dir).join("equity.png");
                    if chart_path.exists() {
                        match fs::read(&chart_path) {
                            Ok(bytes) => {
                                let base64 = BASE64.encode(&bytes);
                                response["report"]["equity_chart_base64"] = json!(base64);
                                response["report"]["equity_chart_format"] = json!("png");
                            }
                            Err(e) => {
                                response["report"]["equity_chart_error"] = json!(format!("Failed to read chart: {}", e));
                            }
                        }
                    } else {
                        response["report"]["equity_chart_error"] = json!("equity.png not found in charts_dir");
                    }
                }
            }

            Ok(json!({
                "content": [{ "type": "text", "text": response.to_string() }],
                "isError": false
            }))
        }
        None => {
            Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "error": format!("Report with id '{}' not found", id)
                }).to_string() }],
                "isError": true
            }))
        }
    }
}

pub async fn handle_get_reports_summary(args: &Value) -> Result<Value> {
    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let filters = ReportFilters {
        expert: args.get("expert").and_then(|v| v.as_str()).map(|s| s.to_string()),
        symbol: args.get("symbol").and_then(|v| v.as_str()).map(|s| s.to_string()),
        timeframe: args.get("timeframe").and_then(|v| v.as_str()).map(|s| s.to_string()),
        verdict: args.get("verdict").and_then(|v| v.as_str()).map(|s| s.to_string()),
        ..Default::default()
    };

    let stats = db.get_stats(&filters)?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "total_count": stats.total_count,
            "profitable_count": stats.profitable_count,
            "pass_verdict_count": stats.pass_verdict_count,
            "fail_verdict_count": stats.fail_verdict_count,
            "marginal_verdict_count": stats.marginal_verdict_count,
            "avg_net_profit": stats.avg_net_profit,
            "avg_profit_factor": stats.avg_profit_factor,
            "avg_max_dd_pct": stats.avg_max_dd_pct,
            "avg_win_rate_pct": stats.avg_win_rate_pct,
            "avg_sharpe_ratio": stats.avg_sharpe_ratio,
            "profitable_rate": if stats.total_count > 0 {
                (stats.profitable_count as f64 / stats.total_count as f64 * 100.0).round()
            } else { 0.0 },
            "pass_rate": if stats.total_count > 0 {
                (stats.pass_verdict_count as f64 / stats.total_count as f64 * 100.0).round()
            } else { 0.0 },
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_best_reports(args: &Value) -> Result<Value> {
    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let sort_by = args.get("sort_by").and_then(|v| v.as_str()).unwrap_or("profit_factor");
    let order = args.get("order").and_then(|v| v.as_str()).unwrap_or("desc");
    let ascending = order == "asc";
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

    let filters = ReportFilters {
        expert: args.get("expert").and_then(|v| v.as_str()).map(|s| s.to_string()),
        symbol: args.get("symbol").and_then(|v| v.as_str()).map(|s| s.to_string()),
        timeframe: args.get("timeframe").and_then(|v| v.as_str()).map(|s| s.to_string()),
        verdict: args.get("verdict").and_then(|v| v.as_str()).map(|s| s.to_string()),
        ..Default::default()
    };

    let entries = db.get_sorted_by(sort_by, ascending, limit, &filters)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "sharpe_ratio": e.sharpe_ratio,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "verdict": e.verdict,
            "tags": e.tags,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "sort_by": sort_by,
            "order": order,
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_reports_by_tags(args: &Value) -> Result<Value> {
    let tags: Vec<String> = args
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .ok_or_else(|| anyhow::anyhow!("tags array is required"))?;

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let entries = db.search_by_tags(&tags, limit)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "verdict": e.verdict,
            "tags": e.tags,
            "notes": e.notes,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "tags": tags,
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_reports_by_date_range(args: &Value) -> Result<Value> {
    let from_start = args.get("from_start").and_then(|v| v.as_str());
    let from_end = args.get("from_end").and_then(|v| v.as_str());
    let to_start = args.get("to_start").and_then(|v| v.as_str());
    let to_end = args.get("to_end").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let entries = db.search_by_date_range(from_start, from_end, to_start, to_end, limit)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "verdict": e.verdict,
            "tags": e.tags,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "from_start": from_start,
            "from_end": from_end,
            "to_start": to_start,
            "to_end": to_end,
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_search_reports_by_notes(args: &Value) -> Result<Value> {
    let query = args.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("query is required"))?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let entries = db.search_by_notes(query, limit)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "verdict": e.verdict,
            "notes": e.notes,
            "tags": e.tags,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "query": query,
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_reports_by_set_file(args: &Value) -> Result<Value> {
    let set_file = args.get("set_file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("set_file is required"))?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    let entries = db.search_by_set_file(set_file, limit)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file_original": e.set_file_original,
            "set_snapshot_path": e.set_snapshot_path,
            "verdict": e.verdict,
            "tags": e.tags,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "set_file": set_file,
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_get_comparable_reports(args: &Value) -> Result<Value> {
    let db = ReportDb::new(&Config::db_path());
    db.init()?;

    // Get expert/symbol/timeframe either from report_id or direct args
    let (expert, symbol, timeframe, exclude_id) = if let Some(id) = args.get("report_id").and_then(|v| v.as_str()) {
        match db.get_by_id(id)? {
            Some(entry) => {
                let exclude = args.get("exclude_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                (entry.expert, entry.symbol, entry.timeframe, exclude.unwrap_or_else(|| id.to_string()))
            }
            None => return Ok(json!({
                "content": [{ "type": "text", "text": json!({
                    "success": false,
                    "error": format!("Report with id '{}' not found", id)
                }).to_string() }],
                "isError": true
            }))
        }
    } else {
        let expert = args.get("expert").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("expert or report_id is required"))?;
        let symbol = args.get("symbol").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("symbol or report_id is required"))?;
        let timeframe = args.get("timeframe").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("timeframe or report_id is required"))?;
        let exclude_id = args.get("exclude_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        (expert.to_string(), symbol.to_string(), timeframe.to_string(), exclude_id.unwrap_or_default())
    };

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let exclude_opt = if exclude_id.is_empty() { None } else { Some(exclude_id.as_str()) };

    let entries = db.get_comparable(&expert, &symbol, &timeframe, exclude_opt, limit)?;

    let reports: Vec<Value> = entries
        .iter()
        .map(|e| json!({
            "id": e.id,
            "expert": e.expert,
            "symbol": e.symbol,
            "timeframe": e.timeframe,
            "from_date": e.from_date,
            "to_date": e.to_date,
            "created_at": e.created_at,
            "net_profit": e.net_profit,
            "profit_factor": e.profit_factor,
            "max_dd_pct": e.max_dd_pct,
            "total_trades": e.total_trades,
            "win_rate_pct": e.win_rate_pct,
            "set_file": e.set_file_original,
            "verdict": e.verdict,
            "tags": e.tags,
        }))
        .collect();

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "expert": expert,
            "symbol": symbol,
            "timeframe": timeframe,
            "exclude_id": exclude_id,
            "matched": reports.len(),
            "reports": reports,
        }).to_string() }],
        "isError": false
    }))
}

pub async fn handle_export_deals_csv(_config: &Config, args: &Value) -> Result<Value> {
    let db = ReportDb::new(&Config::db_path());
    if let Err(e) = db.init() {
        return Ok(json!({ "content": [{ "type": "text", "text": format!("DB error: {}", e) }], "isError": true }));
    }

    let report_id_opt = args.get("report_id").and_then(|v| v.as_str());
    let entry = match report_id_opt {
        Some(id) => db.get_by_id(id)?,
        None => db.get_latest()?,
    };

    let entry = match entry {
        Some(e) => e,
        None => return Ok(json!({ "content": [{ "type": "text", "text": "No report found" }], "isError": true })),
    };

    let deals = db.get_deals(&entry.id)?;
    if deals.is_empty() {
        return Ok(json!({ "content": [{ "type": "text", "text": format!("No deals stored for report {}", entry.id) }], "isError": false }));
    }

    let output_path = match args.get("output_path").and_then(|v| v.as_str()) {
        Some(p) => std::path::PathBuf::from(p),
        None => Path::new(&entry.report_dir).join("deals.csv"),
    };

    let extractor = ReportExtractor::new();
    extractor.write_deals_to_csv(&deals, &output_path)
        .map_err(|e| anyhow::anyhow!("Failed to write CSV: {}", e))?;

    Ok(json!({
        "content": [{ "type": "text", "text": json!({
            "success": true,
            "report_id": entry.id,
            "deals_count": deals.len(),
            "output_path": output_path.to_string_lossy(),
        }).to_string() }],
        "isError": false
    }))
}
