use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::models::Deal;

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
            CREATE TABLE IF NOT EXISTS deals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                report_id TEXT NOT NULL REFERENCES reports(id) ON DELETE CASCADE,
                time TEXT NOT NULL,
                deal TEXT NOT NULL,
                symbol TEXT NOT NULL,
                deal_type TEXT NOT NULL,
                entry TEXT NOT NULL,
                volume REAL NOT NULL,
                price REAL NOT NULL,
                order_id TEXT NOT NULL,
                commission REAL NOT NULL,
                swap REAL NOT NULL,
                profit REAL NOT NULL,
                balance REAL NOT NULL,
                comment TEXT NOT NULL,
                magic TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_deals_report_id ON deals(report_id);
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

    pub fn insert_deals(&self, report_id: &str, deals: &[Deal]) -> Result<()> {
        if deals.is_empty() {
            return Ok(());
        }
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "INSERT INTO deals (report_id, time, deal, symbol, deal_type, entry, volume, price, \
             order_id, commission, swap, profit, balance, comment, magic) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        )?;
        for d in deals {
            stmt.execute(params![
                report_id,
                d.time,
                d.deal,
                d.symbol,
                d.deal_type,
                d.entry,
                d.volume,
                d.price,
                d.order,
                d.commission,
                d.swap,
                d.profit,
                d.balance,
                d.comment,
                d.magic,
            ])?;
        }
        Ok(())
    }

    pub fn get_deals(&self, report_id: &str) -> Result<Vec<Deal>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT time, deal, symbol, deal_type, entry, volume, price, order_id, \
             commission, swap, profit, balance, comment, magic \
             FROM deals WHERE report_id = ? ORDER BY id ASC",
        )?;
        let deals: Vec<Deal> = stmt
            .query_map([report_id], |row| {
                Ok(Deal {
                    time: row.get(0)?,
                    deal: row.get(1)?,
                    symbol: row.get(2)?,
                    deal_type: row.get(3)?,
                    entry: row.get(4)?,
                    volume: row.get(5)?,
                    price: row.get(6)?,
                    order: row.get(7)?,
                    commission: row.get(8)?,
                    swap: row.get(9)?,
                    profit: row.get(10)?,
                    balance: row.get(11)?,
                    comment: row.get(12)?,
                    magic: row.get(13)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(deals)
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

    /// Get the latest report by created_at (most recent first)
    pub fn get_latest(&self) -> Result<Option<ReportEntry>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports ORDER BY created_at DESC LIMIT 1"
        )?;

        let entry = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(23)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
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
            .next();

        Ok(entry)
    }

    /// Get a specific report by its report_dir path (exact match)
    pub fn get_by_report_dir(&self, report_dir: &str) -> Result<Option<ReportEntry>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE report_dir = ?"
        )?;

        let entry = stmt
            .query_map([report_dir], |row| {
                let tags_json: String = row.get(23)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
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
            .next();

        Ok(entry)
    }

    /// Get a specific report by ID
    pub fn get_by_id(&self, id: &str) -> Result<Option<ReportEntry>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE id = ?"
        )?;

        let entry = stmt
            .query_map([id], |row| {
                let tags_json: String = row.get(23)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
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
            .next();

        Ok(entry)
    }

    /// Search reports by tags (at least one tag must match)
    pub fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;
        let mut sql = "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE 1=1"
            .to_string();

        // Build OR conditions for tags - use JSON1 extension for tag matching
        if !tags.is_empty() {
            let tag_conditions: Vec<String> = tags.iter()
                .map(|tag| format!("tags LIKE '%{}%'", tag.replace("'", "''")))
                .collect();
            sql.push_str(&format!(" AND ({})", tag_conditions.join(" OR ")));
        }
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));

        let mut stmt = conn.prepare(&sql)?;
        let entries: Vec<ReportEntry> = stmt
            .query_map([], |row| {
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
            .collect();

        Ok(entries)
    }

    /// Search reports by notes text (case-insensitive LIKE)
    pub fn search_by_notes(&self, query: &str, limit: usize) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;
        let pattern = format!("%{}%", query.replace("'", "''"));
        let mut stmt = conn.prepare(
            &format!("SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE notes LIKE '{}' ORDER BY created_at DESC LIMIT {}",
            pattern, limit)
        )?;

        let entries: Vec<ReportEntry> = stmt
            .query_map([], |row| {
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
            .collect();

        Ok(entries)
    }

    /// Find reports by set file (original or snapshot)
    pub fn search_by_set_file(&self, set_file: &str, limit: usize) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;
        let pattern = format!("%{}%", set_file.replace("'", "''"));
        let mut stmt = conn.prepare(
            &format!("SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE (set_file_original LIKE '{}' OR set_snapshot_path LIKE '{}') \
            ORDER BY created_at DESC LIMIT {}",
            pattern, pattern, limit)
        )?;

        let entries: Vec<ReportEntry> = stmt
            .query_map([], |row| {
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
            .collect();

        Ok(entries)
    }

    /// Search by backtest date range (from_date and to_date fields)
    pub fn search_by_date_range(
        &self,
        from_start: Option<&str>,
        from_end: Option<&str>,
        to_start: Option<&str>,
        to_end: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;
        let mut sql = "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE 1=1"
            .to_string();

        let mut params: Vec<String> = Vec::new();

        if let Some(start) = from_start {
            sql.push_str(" AND from_date >= ?");
            params.push(start.to_string());
        }
        if let Some(end) = from_end {
            sql.push_str(" AND from_date <= ?");
            params.push(end.to_string());
        }
        if let Some(start) = to_start {
            sql.push_str(" AND to_date >= ?");
            params.push(start.to_string());
        }
        if let Some(end) = to_end {
            sql.push_str(" AND to_date <= ?");
            params.push(end.to_string());
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));

        let mut stmt = conn.prepare(&sql)?;
        let entries: Vec<ReportEntry> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
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
            .collect();

        Ok(entries)
    }

    /// Get comparable reports (same expert/symbol/timeframe for comparison)
    pub fn get_comparable(
        &self,
        expert: &str,
        symbol: &str,
        timeframe: &str,
        exclude_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;
        let mut sql = "SELECT id, expert, symbol, timeframe, model, from_date, to_date, \
            created_at, set_file_original, set_snapshot_path, report_dir, charts_dir, \
            net_profit, profit_factor, max_dd_pct, sharpe_ratio, total_trades, \
            win_rate_pct, recovery_factor, deposit, currency, leverage, \
            duration_seconds, tags, notes, verdict \
            FROM reports WHERE expert = ? AND symbol = ? AND timeframe = ?"
            .to_string();

        let mut params: Vec<String> = vec![
            expert.to_string(),
            symbol.to_string(),
            timeframe.to_string(),
        ];

        if let Some(id) = exclude_id {
            sql.push_str(" AND id != ?");
            params.push(id.to_string());
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));

        let mut stmt = conn.prepare(&sql)?;
        let entries: Vec<ReportEntry> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
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
            .collect();

        Ok(entries)
    }

    /// Get reports sorted by a specific metric (for best/worst queries)
    pub fn get_sorted_by(
        &self,
        sort_column: &str,
        ascending: bool,
        limit: usize,
        filters: &ReportFilters,
    ) -> Result<Vec<ReportEntry>> {
        let conn = self.connect()?;

        // Validate sort column to prevent SQL injection
        let valid_columns = ["net_profit", "profit_factor", "max_dd_pct", "win_rate_pct",
                            "sharpe_ratio", "recovery_factor", "total_trades", "created_at"];
        if !valid_columns.contains(&sort_column) {
            return Err(anyhow::anyhow!("Invalid sort column: {}", sort_column));
        }

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
        if let Some(verdict) = &filters.verdict {
            sql.push_str(" AND verdict = ?");
            filter_params.push(verdict.clone());
        }

        // For non-created_at columns, only include rows where that column is not NULL
        if sort_column != "created_at" {
            sql.push_str(&format!(" AND {} IS NOT NULL", sort_column));
        }

        let order = if ascending { "ASC" } else { "DESC" };
        sql.push_str(&format!(" ORDER BY {} {} LIMIT {}", sort_column, order, limit));

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
            .collect();

        Ok(entries)
    }

    /// Get aggregate statistics for reports
    pub fn get_stats(&self, filters: &ReportFilters) -> Result<ReportStats> {
        let conn = self.connect()?;

        let mut sql = "SELECT \
            COUNT(*), \
            AVG(net_profit), \
            AVG(profit_factor), \
            AVG(max_dd_pct), \
            AVG(win_rate_pct), \
            AVG(sharpe_ratio), \
            SUM(CASE WHEN net_profit > 0 THEN 1 ELSE 0 END), \
            SUM(CASE WHEN verdict = 'pass' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN verdict = 'fail' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN verdict = 'marginal' THEN 1 ELSE 0 END) \
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
        if let Some(verdict) = &filters.verdict {
            sql.push_str(" AND verdict = ?");
            filter_params.push(verdict.clone());
        }

        let stats: ReportStats = conn.query_row(
            &sql,
            rusqlite::params_from_iter(filter_params.iter()),
            |row| {
                let total: i64 = row.get(0)?;
                let profitable: Option<i64> = row.get(6)?;
                let pass_count: Option<i64> = row.get(7)?;
                let fail_count: Option<i64> = row.get(8)?;
                let marginal_count: Option<i64> = row.get(9)?;

                Ok(ReportStats {
                    total_count: total as usize,
                    avg_net_profit: row.get(1)?,
                    avg_profit_factor: row.get(2)?,
                    avg_max_dd_pct: row.get(3)?,
                    avg_win_rate_pct: row.get(4)?,
                    avg_sharpe_ratio: row.get(5)?,
                    profitable_count: profitable.unwrap_or(0) as usize,
                    pass_verdict_count: pass_count.unwrap_or(0) as usize,
                    fail_verdict_count: fail_count.unwrap_or(0) as usize,
                    marginal_verdict_count: marginal_count.unwrap_or(0) as usize,
                })
            },
        )?;

        Ok(stats)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ReportStats {
    pub total_count: usize,
    pub avg_net_profit: Option<f64>,
    pub avg_profit_factor: Option<f64>,
    pub avg_max_dd_pct: Option<f64>,
    pub avg_win_rate_pct: Option<f64>,
    pub avg_sharpe_ratio: Option<f64>,
    pub profitable_count: usize,
    pub pass_verdict_count: usize,
    pub fail_verdict_count: usize,
    pub marginal_verdict_count: usize,
}
