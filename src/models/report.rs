use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub deals_csv: PathBuf,
    pub deals_json: PathBuf,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePaths {
    pub metrics: String,
    pub analysis: String,
    pub deals_csv: String,
    pub deals_json: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestStatus {
    pub stage: PipelineStage,
    pub elapsed_seconds: i64,
    pub is_complete: bool,
    pub message: String,
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
}
