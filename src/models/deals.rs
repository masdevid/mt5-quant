use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deal {
    pub time: String,
    pub deal: String,
    pub symbol: String,
    #[serde(rename = "type")]
    pub deal_type: String,
    pub entry: String,
    pub volume: f64,
    pub price: f64,
    pub order: String,
    pub commission: f64,
    pub swap: f64,
    pub profit: f64,
    pub balance: f64,
    pub comment: String,
    pub magic: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DealType {
    Buy,
    Sell,
    Balance,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionPair {
    pub time: String,
    pub deal_type: String,
    pub profit: f64,
    pub volume: f64,
    pub layer: i32,
    pub hold_minutes: Option<f64>,
    pub comment: String,
    pub magic: String,
    pub order: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownEvent {
    pub peak_dd_pct: f64,
    pub start_date: String,
    pub end_date: String,
    pub recovery_date: Option<String>,
    pub recovery_days: Option<i32>,
    pub duration_days: i32,
    pub cause: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyPnl {
    pub month: String,
    pub pnl: f64,
    pub trades: i32,
    pub green: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossSequence {
    pub length: i32,
    pub total_loss: f64,
    pub start: String,
    pub end: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleStats {
    pub total_cycles: i32,
    pub win_rate: f64,
    pub avg_profit: f64,
    pub win_rate_by_depth: HashMap<String, WinRateByDepth>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinRateByDepth {
    pub total: i32,
    pub win_rate: f64,
}
