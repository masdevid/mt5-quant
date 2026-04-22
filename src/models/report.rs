use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-export chrono for BacktestJob
use chrono;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub report_dir: PathBuf,
    pub expert: String,
    pub symbol: String,
    pub timeframe: String,
    pub from_date: String,
    pub to_date: String,
    pub metrics_file: PathBuf,
    pub analysis_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetadata {
    pub expert: String,
    pub symbol: String,
    pub timeframe: String,
    pub from_date: String,
    pub to_date: String,
    pub deposit: f64,
    pub currency: String,
    pub model: i32,
    pub leverage: i32,
    pub set_file: Option<String>,
    pub report_dir: String,
    pub duration_seconds: i64,
    pub files: FilePaths,
    #[serde(default)]
    pub no_trades: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePaths {
    pub metrics: String,
    pub analysis: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestStatus {
    pub stage: PipelineStage,
    pub elapsed_seconds: i64,
    pub is_complete: bool,
    pub message: String,
    pub report_dir: Option<String>,
    pub mt5_running: Option<bool>,
    pub report_found: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PipelineStage {
    Compile,
    Clean,
    Backtest,
    Extract,
    Analyze,
    Done,
    Failed,
}

/// Track a running backtest job for fire-and-poll pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestJob {
    pub report_id: String,
    pub report_dir: String,
    pub expert: String,
    pub symbol: String,
    pub timeframe: String,
    pub launched_at: String,
    pub mt5_pid: Option<u32>,
    pub expected_report_path: String,
    pub timeout_seconds: u64,
}

impl BacktestJob {
    pub fn new(
        report_id: String,
        report_dir: String,
        expert: String,
        symbol: String,
        timeframe: String,
        expected_report_path: String,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            report_id,
            report_dir,
            expert,
            symbol,
            timeframe,
            launched_at: chrono::Utc::now().to_rfc3339(),
            mt5_pid: None,
            expected_report_path,
            timeout_seconds,
        }
    }
}
