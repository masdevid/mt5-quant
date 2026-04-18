use chrono::{DateTime, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::deals::{Deal, DrawdownEvent, LossSequence, MonthlyPnl, PositionPair};
use crate::models::metrics::Metrics;

pub struct DealAnalyzer;

impl DealAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze(&self, deals: &[Deal], metrics: &Metrics) -> AnalysisResult {
        let monthly = self.monthly_pnl(deals);
        let dd_events = self.reconstruct_dd_events(deals, metrics);
        let top_losses = self.top_losses(deals, 10);
        let loss_sequences = self.loss_sequences(deals);
        let pairs = self.position_pairs(deals);
        let bias = self.direction_bias(deals);
        let streak = self.streak_analysis(deals);
        let concurrent = self.concurrent_peak(deals);

        AnalysisResult {
            monthly,
            dd_events,
            top_losses,
            loss_sequences,
            position_pairs: pairs,
            direction_bias: bias,
            streak_analysis: streak,
            concurrent_peak: concurrent,
        }
    }

    pub fn monthly_pnl(&self, deals: &[Deal]) -> Vec<MonthlyPnl> {
        let mut monthly: HashMap<String, (f64, i32)> = HashMap::new();

        for deal in deals {
            let time_str = &deal.time;
            let profit = deal.profit;
            let entry = deal.entry.to_lowercase();

            if !entry.contains("out") && !entry.is_empty() {
                continue;
            }
            if time_str.is_empty() || profit == 0.0 {
                continue;
            }

            if let Some(dt) = Self::parse_datetime(time_str) {
                let month = dt.format("%Y-%m").to_string();
                let entry = monthly.entry(month).or_insert((0.0, 0));
                entry.0 += profit;
                entry.1 += 1;
            }
        }

        let mut result: Vec<MonthlyPnl> = monthly
            .into_iter()
            .map(|(m, (pnl, trades))| MonthlyPnl {
                month: m,
                pnl: (pnl * 100.0).round() / 100.0,
                trades,
                green: pnl >= 0.0,
            })
            .collect();

        result.sort_by(|a, b| a.month.cmp(&b.month));
        result
    }

    pub fn reconstruct_dd_events(&self, deals: &[Deal], _metrics: &Metrics) -> Vec<DrawdownEvent> {
        let mut balance_curve = Vec::new();
        let mut peak_balance: f64 = 0.0;
        let mut initial_balance: Option<f64> = None;

        for deal in deals {
            let balance = deal.balance;
            if balance > 0.0 {
                if initial_balance.is_none() {
                    initial_balance = Some(balance);
                }
                peak_balance = peak_balance.max(balance);
                let dd_pct = if peak_balance > 0.0 {
                    (peak_balance - balance) / peak_balance * 100.0
                } else {
                    0.0
                };
                balance_curve.push((deal.time.clone(), balance, dd_pct, deal.profit, deal.comment.clone()));
            }
        }

        if balance_curve.is_empty() {
            return Vec::new();
        }

        let mut events = Vec::new();
        let mut in_dd = false;
        let mut dd_start_idx = 0_usize;
        let threshold = 1.0;

        for (i, (_, _, dd_pct, _, _)) in balance_curve.iter().enumerate() {
            if !in_dd && *dd_pct > threshold {
                in_dd = true;
                dd_start_idx = i;
            } else if in_dd && *dd_pct < threshold {
                let peak_idx = (dd_start_idx..=i)
                    .max_by(|a, b| {
                        let (_, _, dd_pct_a, _, _) = balance_curve[*a];
                        let (_, _, dd_pct_b, _, _) = balance_curve[*b];
                        dd_pct_a.partial_cmp(&dd_pct_b).unwrap()
                    })
                    .unwrap_or(dd_start_idx);

                let event = self.build_dd_event(&balance_curve, dd_start_idx, peak_idx, Some(i));
                if event.peak_dd_pct > 1.0 {
                    events.push(event);
                }
                in_dd = false;
            }
        }

        if in_dd {
            let peak_idx = (dd_start_idx..balance_curve.len())
                .max_by(|a, b| {
                    let (_, _, dd_pct_a, _, _) = balance_curve[*a];
                    let (_, _, dd_pct_b, _, _) = balance_curve[*b];
                    dd_pct_a.partial_cmp(&dd_pct_b).unwrap()
                })
                .unwrap_or(dd_start_idx);

            let event = self.build_dd_event(&balance_curve, dd_start_idx, peak_idx, None);
            if event.peak_dd_pct > 1.0 {
                events.push(event);
            }
        }

        events.sort_by(|a, b| b.peak_dd_pct.partial_cmp(&a.peak_dd_pct).unwrap());
        events.truncate(10);
        events
    }

    fn build_dd_event(&self, curve: &[(String, f64, f64, f64, String)], start_idx: usize, peak_idx: usize, recovery_idx: Option<usize>) -> DrawdownEvent {
        let (start_time, _, _, _, _) = &curve[start_idx];
        let (peak_time, _, peak_dd, _, _) = &curve[peak_idx];

        let mut event = DrawdownEvent {
            peak_dd_pct: (*peak_dd * 100.0).round() / 100.0,
            start_date: Self::extract_date(start_time),
            end_date: Self::extract_date(peak_time),
            recovery_date: None,
            recovery_days: None,
            duration_days: 0,
            cause: "unknown".to_string(),
        };

        if let Some(rec_idx) = recovery_idx {
            let (rec_time, _, _, _, _) = &curve[rec_idx];
            event.recovery_date = Some(Self::extract_date(rec_time));
            
            if let (Ok(start_dt), Ok(rec_dt)) = (
                chrono::NaiveDate::parse_from_str(&event.start_date, "%Y-%m-%d"),
                chrono::NaiveDate::parse_from_str(event.recovery_date.as_ref().unwrap(), "%Y-%m-%d")
            ) {
                event.recovery_days = Some((rec_dt - start_dt).num_days() as i32);
            }
        }

        if let (Ok(start_dt), Ok(end_dt)) = (
            chrono::NaiveDate::parse_from_str(&event.start_date, "%Y-%m-%d"),
            chrono::NaiveDate::parse_from_str(&event.end_date, "%Y-%m-%d")
        ) {
            event.duration_days = (end_dt - start_dt).num_days() as i32;
        }

        event
    }

    pub fn top_losses(&self, deals: &[Deal], n: usize) -> Vec<LossEntry> {
        let mut losses: Vec<LossEntry> = deals
            .iter()
            .filter(|d| d.profit < 0.0)
            .map(|d| LossEntry {
                date: Self::extract_date(&d.time),
                loss_usd: (d.profit * 100.0).round() / 100.0,
                comment: d.comment.clone(),
                grid_depth_at_close: self.extract_layer(&d.comment),
                volume: d.volume,
            })
            .collect();

        losses.sort_by(|a, b| a.loss_usd.partial_cmp(&b.loss_usd).unwrap());
        losses.truncate(n);
        losses
    }

    pub fn loss_sequences(&self, deals: &[Deal]) -> Vec<LossSequence> {
        let closed: Vec<&Deal> = deals
            .iter()
            .filter(|d| d.entry.to_lowercase().contains("out") && d.profit != 0.0)
            .collect();

        if closed.is_empty() {
            return Vec::new();
        }

        let mut sequences = Vec::new();
        let mut current_seq: Vec<&Deal> = Vec::new();

        for deal in closed {
            if deal.profit < 0.0 {
                current_seq.push(deal);
            } else {
                if current_seq.len() >= 2 {
                    let total: f64 = current_seq.iter().map(|d| d.profit).sum();
                    sequences.push(LossSequence {
                        length: current_seq.len() as i32,
                        total_loss: (total * 100.0).round() / 100.0,
                        start: Self::extract_date(&current_seq[0].time),
                        end: Self::extract_date(&current_seq[current_seq.len() - 1].time),
                    });
                }
                current_seq.clear();
            }
        }

        if current_seq.len() >= 2 {
            let total: f64 = current_seq.iter().map(|d| d.profit).sum();
            sequences.push(LossSequence {
                length: current_seq.len() as i32,
                total_loss: (total * 100.0).round() / 100.0,
                start: Self::extract_date(&current_seq[0].time),
                end: Self::extract_date(&current_seq[current_seq.len() - 1].time),
            });
        }

        sequences.sort_by(|a, b| a.total_loss.partial_cmp(&b.total_loss).unwrap());
        sequences.truncate(5);
        sequences
    }

    pub fn position_pairs(&self, deals: &[Deal]) -> Vec<PositionPair> {
        let mut open_pos: HashMap<String, &Deal> = HashMap::new();
        let mut pairs = Vec::new();

        for deal in deals {
            let order = &deal.order;
            let entry = deal.entry.to_lowercase();

            if entry.contains("in") && !entry.contains("out") {
                open_pos.insert(order.clone(), deal);
            } else if entry.contains("out") && deal.profit != 0.0 {
                if let Some(in_deal) = open_pos.remove(order) {
                    if let (Some(dt_out), Some(dt_in)) = (
                        Self::parse_datetime(&deal.time),
                        Self::parse_datetime(&in_deal.time)
                    ) {
                        let hold_minutes = (dt_out - dt_in).num_seconds() as f64 / 60.0;

                        pairs.push(PositionPair {
                            time: deal.time.clone(),
                            deal_type: deal.deal_type.clone(),
                            profit: deal.profit,
                            volume: deal.volume,
                            layer: self.extract_layer(&deal.comment),
                            hold_minutes: Some((hold_minutes * 10.0).round() / 10.0),
                            comment: deal.comment.clone(),
                            magic: deal.magic.clone().unwrap_or_default(),
                            order: order.clone(),
                        });
                    }
                }
            }
        }

        pairs
    }

    pub fn direction_bias(&self, deals: &[Deal]) -> HashMap<String, DirectionStats> {
        let mut stats: HashMap<String, (i32, i32, f64)> = HashMap::new();
        stats.insert("buy".to_string(), (0, 0, 0.0));
        stats.insert("sell".to_string(), (0, 0, 0.0));

        for deal in deals {
            let entry = deal.entry.to_lowercase();
            if !entry.contains("out") || deal.profit == 0.0 {
                continue;
            }

            let d = deal.deal_type.to_lowercase();
            if let Some((trades, wins, total_pnl)) = stats.get_mut(&d) {
                *trades += 1;
                *total_pnl += deal.profit;
                if deal.profit > 0.0 {
                    *wins += 1;
                }
            }
        }

        stats
            .into_iter()
            .filter(|(_, (trades, _, _))| *trades > 0)
            .map(|(d, (trades, wins, total_pnl))| {
                let avg_pnl = if trades > 0 { total_pnl / trades as f64 } else { 0.0 };
                (d, DirectionStats {
                    trades,
                    win_rate: ((wins as f64 / trades as f64) * 1000.0).round() / 10.0,
                    total_pnl: (total_pnl * 100.0).round() / 100.0,
                    avg_pnl: (avg_pnl * 100.0).round() / 100.0,
                })
            })
            .collect()
    }

    pub fn streak_analysis(&self, deals: &[Deal]) -> StreakAnalysis {
        let closed: Vec<&Deal> = deals
            .iter()
            .filter(|d| d.entry.to_lowercase().contains("out") && d.profit != 0.0)
            .collect();

        if closed.is_empty() {
            return StreakAnalysis::default();
        }

        let (mut max_win_streak, mut max_loss_streak) = (0, 0);
        let (mut cur_win, mut cur_loss) = (0, 0);
        let (mut max_win_start, mut max_win_end) = (String::new(), String::new());
        let (mut max_loss_start, mut max_loss_end) = (String::new(), String::new());
        let (mut win_run_start, mut loss_run_start) = (String::new(), String::new());

        for deal in closed.iter() {
            let profit = deal.profit;
            let t = Self::extract_date(&deal.time);

            if profit > 0.0 {
                if cur_win == 0 {
                    win_run_start = t.clone();
                }
                cur_win += 1;
                cur_loss = 0;
                if cur_win > max_win_streak {
                    max_win_streak = cur_win;
                    max_win_start = win_run_start.clone();
                    max_win_end = t.clone();
                }
            } else {
                if cur_loss == 0 {
                    loss_run_start = t.clone();
                }
                cur_loss += 1;
                cur_win = 0;
                if cur_loss > max_loss_streak {
                    max_loss_streak = cur_loss;
                    max_loss_start = loss_run_start.clone();
                    max_loss_end = t.clone();
                }
            }
        }

        let last = closed.last().unwrap();
        StreakAnalysis {
            max_win_streak,
            max_win_start,
            max_win_end,
            max_loss_streak,
            max_loss_start,
            max_loss_end,
            current_streak: if last.profit > 0.0 { cur_win } else { cur_loss },
            current_streak_type: if last.profit > 0.0 { "win".to_string() } else { "loss".to_string() },
        }
    }

    pub fn concurrent_peak(&self, deals: &[Deal]) -> ConcurrentPeak {
        let mut events: Vec<(DateTime<chrono::Utc>, i32, &Deal)> = Vec::new();

        for deal in deals {
            let entry = deal.entry.to_lowercase();
            if let Some(dt) = Self::parse_datetime(&deal.time) {
                if entry.contains("in") && !entry.contains("out") {
                    events.push((dt, 1, deal));
                } else if entry.contains("out") {
                    events.push((dt, -1, deal));
                }
            }
        }

        events.sort_by(|a, b| a.0.cmp(&b.0));

        let mut count = 0;
        let mut peak = 0;
        let mut peak_time = String::new();

        for (_dt, delta, deal) in events {
            count = (count + delta).max(0);
            if count > peak {
                peak = count;
                peak_time = deal.time.clone();
            }
        }

        ConcurrentPeak { peak_open: peak, peak_time }
    }

    fn parse_datetime(time_str: &str) -> Option<DateTime<chrono::Utc>> {
        let s = time_str.trim();
        
        let formats = [
            "%Y.%m.%d %H:%M:%S",
            "%Y-%m-%d %H:%M:%S",
            "%Y.%m.%d",
            "%Y-%m-%d",
        ];

        for fmt in &formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&s[..s.len().min(19)], fmt) {
                return Some(DateTime::from_naive_utc_and_offset(dt, chrono::Utc));
            }
        }

        None
    }

    fn extract_date(time_str: &str) -> String {
        if time_str.len() >= 10 {
            time_str[..10].replace('.', "-")
        } else {
            time_str.to_string()
        }
    }

    fn extract_layer(&self, comment: &str) -> i32 {
        let re = regex::Regex::new(r"[Ll]ayer\s*#?(\d+)").ok();
        if let Some(re) = re {
            re.captures(comment)
                .and_then(|cap| cap.get(1))
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0)
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub monthly: Vec<MonthlyPnl>,
    pub dd_events: Vec<DrawdownEvent>,
    pub top_losses: Vec<LossEntry>,
    pub loss_sequences: Vec<LossSequence>,
    pub position_pairs: Vec<PositionPair>,
    pub direction_bias: HashMap<String, DirectionStats>,
    pub streak_analysis: StreakAnalysis,
    pub concurrent_peak: ConcurrentPeak,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossEntry {
    pub date: String,
    pub loss_usd: f64,
    pub comment: String,
    pub grid_depth_at_close: i32,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionStats {
    pub trades: i32,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreakAnalysis {
    pub max_win_streak: i32,
    pub max_win_start: String,
    pub max_win_end: String,
    pub max_loss_streak: i32,
    pub max_loss_start: String,
    pub max_loss_end: String,
    pub current_streak: i32,
    pub current_streak_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrentPeak {
    pub peak_open: i32,
    pub peak_time: String,
}
