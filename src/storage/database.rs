use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ReportEntry {
    pub id: String,
    pub expert: String,
    pub symbol: String,
    pub timeframe: String,
    pub model: i64,
    pub from_date: String,
    pub to_date: String,
    pub created_at: String,
    pub set_file_original: Option<String>,
    pub set_snapshot_path: Option<String>,
    pub report_dir: String,
    pub charts_dir: Option<String>,
    pub net_profit: Option<f64>,
    pub profit_factor: Option<f64>,
    pub max_dd_pct: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub total_trades: Option<i64>,
    pub win_rate_pct: Option<f64>,
    pub recovery_factor: Option<f64>,
    pub deposit: Option<f64>,
    pub currency: Option<String>,
    pub leverage: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub verdict: Option<String>,
}

#[derive(Debug, Default)]
pub struct ReportFilters {
    pub expert: Option<String>,
    pub symbol: Option<String>,
    pub timeframe: Option<String>,
    pub created_after: Option<String>,
    pub min_profit: Option<f64>,
    pub max_dd: Option<f64>,
    pub verdict: Option<String>,
}

pub struct ReportDb {
    db_path: PathBuf,
}

impl ReportDb {
    pub fn new(db_path: &Path) -> Self {
        Self {
            db_path: db_path.to_path_buf(),
        }
    }

    fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(conn)
    }

    pub fn init(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = self.connect()?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS reports (
                id TEXT PRIMARY KEY,
                expert TEXT NOT NULL,
                symbol TEXT NOT NULL,
                timeframe TEXT NOT NULL,
                model INTEGER NOT NULL DEFAULT 0,
                from_date TEXT NOT NULL DEFAULT '',
                to_date TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                set_file_original TEXT,
                set_snapshot_path TEXT,
                report_dir TEXT NOT NULL,
                charts_dir TEXT,
                net_profit REAL,
                profit_factor REAL,
                max_dd_pct REAL,
                sharpe_ratio REAL,
                total_trades INTEGER,
                win_rate_pct REAL,
                recovery_factor REAL,
                deposit REAL,
                currency TEXT,
                leverage INTEGER,
                duration_seconds INTEGER,
                tags TEXT DEFAULT '[]',
                notes TEXT,
                verdict TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_reports_expert ON reports(expert);
            CREATE INDEX IF NOT EXISTS idx_reports_symbol ON reports(symbol);
            CREATE INDEX IF NOT EXISTS idx_reports_created_at ON reports(created_at DESC);
        ")?;
        Ok(())
    }

    pub fn insert(&self, entry: &ReportEntry) -> Result<()> {
        let conn = self.connect()?;
        let tags_json = serde_json::to_string(&entry.tags)?;
        conn.execute(
            "INSERT OR REPLACE INTO reports
             (id, expert, symbol, timeframe, model, from_date, to_date, created_at,
              set_file_original, set_snapshot_path, report_dir, charts_dir,
              net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades,
              win_rate_pct, recovery_factor, deposit, currency, leverage,
              duration_seconds, tags, notes, verdict)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)",
            params![
                entry.id,
                entry.expert,
                entry.symbol,
                entry.timeframe,
                entry.model,
                entry.from_date,
                entry.to_date,
                entry.created_at,
                entry.set_file_original,
                entry.set_snapshot_path,
                entry.report_dir,
                entry.charts_dir,
                entry.net_profit,
                entry.profit_factor,
                entry.max_dd_pct,
                entry.sharpe_ratio,
                entry.total_trades,
                entry.win_rate_pct,
                entry.recovery_factor,
                entry.deposit,
                entry.currency,
                entry.leverage,
                entry.duration_seconds,
                tags_json,
                entry.notes,
                entry.verdict,
            ],
        )?;
        Ok(())
    }

    pub fn list(&self, limit: usize, filters: &ReportFilters) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;

        let mut sql = "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE 1=1"
            .to_string();

        let mut filter_params: Vec<String> = Vec::new();

        if let Some(ea) = &filters.expert {
            sql.push_str(" AND expert LIKE ?");
            filter_params.push(format!("%{}%", ea));
        }
        if let Some(sym) = &filters.symbol {
            sql.push_str(" AND symbol = ?");
            filter_params.push(sym.clone());
        }
        if let Some(tf) = &filters.timeframe {
            sql.push_str(" AND timeframe = ?");
            filter_params.push(tf.clone());
        }
        if let Some(after) = &filters.created_after {
            sql.push_str(" AND created_at >= ?");
            filter_params.push(after.clone());
        }
        if let Some(verdict) = &filters.verdict {
            sql.push_str(" AND verdict = ?");
            filter_params.push(verdict.clone());
        }

        // Overfetch for in-memory numeric filters, then truncate
        let fetch_limit = if filters.min_profit.is_some() || filters.max_dd.is_some() {
            limit * 4
        } else {
            limit
        };
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", fetch_limit));

        let mut stmt = conn.prepare(&sql)?;
        let entries: Vec<ReportEntry> = stmt
            .query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
                let tags_str: String = row.get(23).unwrap_or_else(|_| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                Ok(ReportEntry {
                    id: row.get(0)?,
                    expert: row.get(1)?,
                    symbol: row.get(2)?,
                    timeframe: row.get(3)?,
                    model: row.get(4)?,
                    from_date: row.get(5)?,
                    to_date: row.get(6)?,
                    created_at: row.get(7)?,
                    set_file_original: row.get(8)?,
                    set_snapshot_path: row.get(9)?,
                    report_dir: row.get(10)?,
                    charts_dir: row.get(11)?,
                    net_profit: row.get(12)?,
                    profit_factor: row.get(13)?,
                    max_dd_pct: row.get(14)?,
                    sharpe_ratio: row.get(15)?,
                    total_trades: row.get(16)?,
                    win_rate_pct: row.get(17)?,
                    recovery_factor: row.get(18)?,
                    deposit: row.get(19)?,
                    currency: row.get(20)?,
                    leverage: row.get(21)?,
                    duration_seconds: row.get(22)?,
                    tags,
                    notes: row.get(24)?,
                    verdict: row.get(25)?,
                })
            })?
            .filter_map(|r| r.ok())
            .filter(|e| {
                if let Some(min_profit) = filters.min_profit {
                    if e.net_profit.unwrap_or(f64::MIN) < min_profit {
                        return false;
                    }
                }
                if let Some(max_dd) = filters.max_dd {
                    if e.max_dd_pct.unwrap_or(100.0) > max_dd {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .collect();

        Ok(entries)
    }

    pub fn annotate(
        &self,
        id: &str,
        notes: Option<&str>,
        tags: Option<Vec<String>>,
        verdict: Option<&str>,
    ) -> Result<bool> {
        let conn = self.connect()?;
        let mut sets: Vec<String> = Vec::new();
        let mut param_vals: Vec<String> = Vec::new();

        if let Some(n) = notes {
            sets.push("notes = ?".to_string());
            param_vals.push(n.to_string());
        }
        if let Some(t) = tags {
            sets.push("tags = ?".to_string());
            param_vals.push(serde_json::to_string(&t).unwrap_or_default());
        }
        if let Some(v) = verdict {
            sets.push("verdict = ?".to_string());
            param_vals.push(v.to_string());
        }

        if sets.is_empty() {
            return Ok(false);
        }

        param_vals.push(id.to_string());
        let sql = format!("UPDATE reports SET {} WHERE id = ?", sets.join(", "));
        let changed =
            conn.execute(&sql, rusqlite::params_from_iter(param_vals.iter()))?;
        Ok(changed > 0)
    }

    /// Returns (id, report_dir, charts_dir) for the oldest entries beyond keep_last.
    pub fn list_purgeable(
        &self,
        keep_last: usize,
    ) -> Result<Vec<(String, String, Option<String>)>> {
        let conn = self.connect()?;
        let count: usize =
            conn.query_row("SELECT COUNT(*) FROM reports", [], |r| r.get(0))?;

        if count <= keep_last {
            return Ok(Vec::new());
        }

        let to_delete = count - keep_last;
        let mut stmt = conn.prepare(
            "SELECT id, report_dir, charts_dir FROM reports ORDER BY created_at ASC LIMIT ?",
        )?;

        let rows: Vec<(String, String, Option<String>)> = stmt
            .query_map(params![to_delete as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    pub fn delete_entry(&self, id: &str) -> Result<Option<String>> {
        let conn = self.connect()?;
        let report_dir: Option<String> = conn
            .query_row(
                "SELECT report_dir FROM reports WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .ok();

        conn.execute("DELETE FROM reports WHERE id = ?", params![id])?;
        Ok(report_dir)
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.connect()?;
        let n: usize = conn.query_row("SELECT COUNT(*) FROM reports", [], |r| r.get(0))?;
        Ok(n)
    }
}
