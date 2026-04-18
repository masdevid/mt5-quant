use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use crate::models::Config;
use crate::storage::{ReportDb, ReportFilters};

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
