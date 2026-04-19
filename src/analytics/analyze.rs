use chrono::{DateTime, Datelike, NaiveDateTime, Timelike};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitDistribution {
    pub small_wins: i32,
    pub medium_wins: i32,
    pub large_wins: i32,
    pub small_losses: i32,
    pub medium_losses: i32,
    pub large_losses: i32,
    pub small_win_pnl: f64,
    pub medium_win_pnl: f64,
    pub large_win_pnl: f64,
    pub small_loss_pnl: f64,
    pub medium_loss_pnl: f64,
    pub large_loss_pnl: f64,
    pub buckets: Vec<ProfitBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitBucket {
    pub range: String,
    pub min: f64,
    pub max: f64,
    pub count: i32,
    pub total_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePerformance {
    pub by_hour: Vec<HourPerformance>,
    pub by_day: Vec<DayPerformance>,
    pub best_hour: i32,
    pub worst_hour: i32,
    pub best_day: String,
    pub worst_day: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourPerformance {
    pub hour: i32,
    pub trades: i32,
    pub wins: i32,
    pub total_pnl: f64,
    pub win_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayPerformance {
    pub day: String,
    pub day_num: i32,
    pub trades: i32,
    pub wins: i32,
    pub total_pnl: f64,
    pub win_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldTimeAnalysis {
    pub avg_hold_minutes: f64,
    pub median_hold_minutes: f64,
    pub buckets: Vec<HoldTimeBucket>,
    pub correlation_with_profit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldTimeBucket {
    pub range: String,
    pub min_minutes: f64,
    pub max_minutes: f64,
    pub count: i32,
    pub avg_profit: f64,
    pub total_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPerformance {
    pub layer: i32,
    pub trades: i32,
    pub wins: i32,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub avg_volume: f64,
    pub avg_profit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAnalysis {
    pub correlation_with_profit: f64,
    pub by_volume_bucket: Vec<VolumeBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeBucket {
    pub volume_range: String,
    pub min_volume: f64,
    pub max_volume: f64,
    pub trades: i32,
    pub avg_profit: f64,
    pub total_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAnalysis {
    pub total_commission: f64,
    pub total_swap: f64,
    pub commission_pct_of_profit: f64,
    pub swap_pct_of_profit: f64,
    pub avg_commission_per_trade: f64,
    pub avg_swap_per_trade: f64,
    pub net_profit_before_costs: f64,
    pub cost_impact_on_win_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyAnalysis {
    pub profit_per_hour: f64,
    pub profit_per_day: f64,
    pub profit_per_trade_hour: f64,
    pub avg_trade_duration_hours: f64,
    pub annualized_return_pct: f64,
    pub trades_per_day: f64,
}

impl DealAnalyzer {
    pub fn profit_distribution(&self, deals: &[Deal]) -> ProfitDistribution {
        let closed: Vec<&Deal> = deals
            .iter()
            .filter(|d| d.entry.to_lowercase().contains("out") && d.profit != 0.0)
            .collect();

        let mut small_wins = 0;
        let mut medium_wins = 0;
        let mut large_wins = 0;
        let mut small_losses = 0;
        let mut medium_losses = 0;
        let mut large_losses = 0;
        let mut small_win_pnl = 0.0;
        let mut medium_win_pnl = 0.0;
        let mut large_win_pnl = 0.0;
        let mut small_loss_pnl = 0.0;
        let mut medium_loss_pnl = 0.0;
        let mut large_loss_pnl = 0.0;

        for deal in &closed {
            let profit = deal.profit;
            if profit > 0.0 {
                if profit < 50.0 {
                    small_wins += 1;
                    small_win_pnl += profit;
                } else if profit < 200.0 {
                    medium_wins += 1;
                    medium_win_pnl += profit;
                } else {
                    large_wins += 1;
                    large_win_pnl += profit;
                }
            } else {
                let loss = profit.abs();
                if loss < 50.0 {
                    small_losses += 1;
                    small_loss_pnl += profit;
                } else if loss < 200.0 {
                    medium_losses += 1;
                    medium_loss_pnl += profit;
                } else {
                    large_losses += 1;
                    large_loss_pnl += profit;
                }
            }
        }

        // Create detailed buckets
        let bucket_ranges = [
            (-999999.0, -500.0, "Loss $500+"),
            (-500.0, -200.0, "Loss $200-500"),
            (-200.0, -50.0, "Loss $50-200"),
            (-50.0, 0.0, "Loss $0-50"),
            (0.0, 50.0, "Win $0-50"),
            (50.0, 200.0, "Win $50-200"),
            (200.0, 500.0, "Win $200-500"),
            (500.0, 999999.0, "Win $500+"),
        ];

        let mut buckets: Vec<ProfitBucket> = bucket_ranges
            .iter()
            .map(|(min, max, range)| {
                let count = closed
                    .iter()
                    .filter(|d| d.profit >= *min && d.profit < *max)
                    .count() as i32;
                let total_pnl: f64 = closed
                    .iter()
                    .filter(|d| d.profit >= *min && d.profit < *max)
                    .map(|d| d.profit)
                    .sum();
                ProfitBucket {
                    range: range.to_string(),
                    min: *min,
                    max: *max,
                    count,
                    total_pnl: (total_pnl * 100.0).round() / 100.0,
                }
            })
            .collect();

        // Remove empty buckets
        buckets.retain(|b| b.count > 0);

        ProfitDistribution {
            small_wins,
            medium_wins,
            large_wins,
            small_losses,
            medium_losses,
            large_losses,
            small_win_pnl: (small_win_pnl * 100.0).round() / 100.0,
            medium_win_pnl: (medium_win_pnl * 100.0).round() / 100.0,
            large_win_pnl: (large_win_pnl * 100.0).round() / 100.0,
            small_loss_pnl: (small_loss_pnl * 100.0).round() / 100.0,
            medium_loss_pnl: (medium_loss_pnl * 100.0).round() / 100.0,
            large_loss_pnl: (large_loss_pnl * 100.0).round() / 100.0,
            buckets,
        }
    }

    pub fn time_performance(&self, deals: &[Deal]) -> TimePerformance {
        let closed: Vec<&Deal> = deals
            .iter()
            .filter(|d| d.entry.to_lowercase().contains("out") && d.profit != 0.0)
            .collect();

        let mut hourly: HashMap<i32, (i32, i32, f64)> = HashMap::new();
        let mut daily: HashMap<String, (i32, i32, f64, i32)> = HashMap::new();

        for deal in &closed {
            if let Some(dt) = Self::parse_datetime(&deal.time) {
                let hour = dt.hour() as i32;
                let day_num = dt.weekday().num_days_from_monday() as i32;
                let day_name = match dt.weekday() {
                    chrono::Weekday::Mon => "Mon",
                    chrono::Weekday::Tue => "Tue",
                    chrono::Weekday::Wed => "Wed",
                    chrono::Weekday::Thu => "Thu",
                    chrono::Weekday::Fri => "Fri",
                    chrono::Weekday::Sat => "Sat",
                    chrono::Weekday::Sun => "Sun",
                }.to_string();

                let entry = hourly.entry(hour).or_insert((0, 0, 0.0));
                entry.0 += 1;
                entry.2 += deal.profit;
                if deal.profit > 0.0 {
                    entry.1 += 1;
                }

                let day_entry = daily.entry(day_name.clone()).or_insert((0, 0, 0.0, day_num));
                day_entry.0 += 1;
                day_entry.2 += deal.profit;
                if deal.profit > 0.0 {
                    day_entry.1 += 1;
                }
            }
        }

        let mut by_hour: Vec<HourPerformance> = hourly
            .into_iter()
            .map(|(hour, (trades, wins, total_pnl))| HourPerformance {
                hour,
                trades,
                wins,
                total_pnl: (total_pnl * 100.0).round() / 100.0,
                win_rate: if trades > 0 { (wins as f64 / trades as f64 * 1000.0).round() / 10.0 } else { 0.0 },
            })
            .collect();
        by_hour.sort_by_key(|h| h.hour);

        let mut by_day: Vec<DayPerformance> = daily
            .into_iter()
            .map(|(day, (trades, wins, total_pnl, day_num))| DayPerformance {
                day: day.clone(),
                day_num,
                trades,
                wins,
                total_pnl: (total_pnl * 100.0).round() / 100.0,
                win_rate: if trades > 0 { (wins as f64 / trades as f64 * 1000.0).round() / 10.0 } else { 0.0 },
            })
            .collect();
        by_day.sort_by_key(|d| d.day_num);

        let best_hour = by_hour.iter().max_by(|a, b| a.total_pnl.partial_cmp(&b.total_pnl).unwrap()).map(|h| h.hour).unwrap_or(-1);
        let worst_hour = by_hour.iter().min_by(|a, b| a.total_pnl.partial_cmp(&b.total_pnl).unwrap()).map(|h| h.hour).unwrap_or(-1);
        let best_day = by_day.iter().max_by(|a, b| a.total_pnl.partial_cmp(&b.total_pnl).unwrap()).map(|d| d.day.clone()).unwrap_or_default();
        let worst_day = by_day.iter().min_by(|a, b| a.total_pnl.partial_cmp(&b.total_pnl).unwrap()).map(|d| d.day.clone()).unwrap_or_default();

        TimePerformance {
            by_hour,
            by_day,
            best_hour,
            worst_hour,
            best_day,
            worst_day,
        }
    }

    pub fn hold_time_analysis(&self, deals: &[Deal]) -> HoldTimeAnalysis {
        let mut hold_times: Vec<(f64, f64)> = Vec::new(); // (hold_minutes, profit)
        let mut open_pos: HashMap<String, DateTime<chrono::Utc>> = HashMap::new();

        for deal in deals {
            let entry = deal.entry.to_lowercase();
            if let Some(dt) = Self::parse_datetime(&deal.time) {
                if entry.contains("in") && !entry.contains("out") {
                    open_pos.insert(deal.order.clone(), dt);
                } else if entry.contains("out") && deal.profit != 0.0 {
                    if let Some(in_time) = open_pos.remove(&deal.order) {
                        let hold_minutes = (dt - in_time).num_seconds() as f64 / 60.0;
                        if hold_minutes > 0.0 {
                            hold_times.push((hold_minutes, deal.profit));
                        }
                    }
                }
            }
        }

        if hold_times.is_empty() {
            return HoldTimeAnalysis {
                avg_hold_minutes: 0.0,
                median_hold_minutes: 0.0,
                buckets: vec![],
                correlation_with_profit: 0.0,
            };
        }

        let avg_hold = hold_times.iter().map(|(h, _)| *h).sum::<f64>() / hold_times.len() as f64;
        let mut sorted_hold: Vec<f64> = hold_times.iter().map(|(h, _)| *h).collect();
        sorted_hold.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_hold = sorted_hold[sorted_hold.len() / 2];

        // Calculate correlation
        let n = hold_times.len() as f64;
        let sum_x = hold_times.iter().map(|(h, _)| *h).sum::<f64>();
        let sum_y = hold_times.iter().map(|(_, p)| *p).sum::<f64>();
        let sum_xy = hold_times.iter().map(|(h, p)| h * p).sum::<f64>();
        let sum_x2 = hold_times.iter().map(|(h, _)| h * h).sum::<f64>();
        let sum_y2 = hold_times.iter().map(|(_, p)| p * p).sum::<f64>();

        let correlation = if n > 1.0 {
            let numerator = n * sum_xy - sum_x * sum_y;
            let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
            if denominator > 0.0 { numerator / denominator } else { 0.0 }
        } else {
            0.0
        };

        // Create buckets
        let bucket_defs = [
            (0.0, 15.0, "< 15 min"),
            (15.0, 60.0, "15-60 min"),
            (60.0, 240.0, "1-4 hours"),
            (240.0, 1440.0, "4-24 hours"),
            (1440.0, 10080.0, "1-7 days"),
            (10080.0, 999999.0, "> 7 days"),
        ];

        let buckets: Vec<HoldTimeBucket> = bucket_defs
            .iter()
            .map(|(min, max, range)| {
                let bucket_deals: Vec<(f64, f64)> = hold_times
                    .iter()
                    .filter(|(h, _)| *h >= *min && *h < *max)
                    .cloned()
                    .collect();
                let count = bucket_deals.len() as i32;
                let total_pnl: f64 = bucket_deals.iter().map(|(_, p)| *p).sum();
                let avg_profit = if count > 0 { total_pnl / count as f64 } else { 0.0 };

                HoldTimeBucket {
                    range: range.to_string(),
                    min_minutes: *min,
                    max_minutes: *max,
                    count,
                    avg_profit: (avg_profit * 100.0).round() / 100.0,
                    total_pnl: (total_pnl * 100.0).round() / 100.0,
                }
            })
            .collect();

        HoldTimeAnalysis {
            avg_hold_minutes: (avg_hold * 10.0).round() / 10.0,
            median_hold_minutes: (median_hold * 10.0).round() / 10.0,
            buckets,
            correlation_with_profit: (correlation * 1000.0).round() / 1000.0,
        }
    }

    pub fn layer_performance(&self, deals: &[Deal]) -> Vec<LayerPerformance> {
        let mut layer_stats: HashMap<i32, (i32, i32, f64, f64)> = HashMap::new();

        for deal in deals {
            let entry = deal.entry.to_lowercase();
            if entry.contains("out") && deal.profit != 0.0 {
                let layer = self.extract_layer(&deal.comment);
                let stats = layer_stats.entry(layer).or_insert((0, 0, 0.0, 0.0));
                stats.0 += 1;
                stats.2 += deal.profit;
                stats.3 += deal.volume;
                if deal.profit > 0.0 {
                    stats.1 += 1;
                }
            }
        }

        let mut result: Vec<LayerPerformance> = layer_stats
            .into_iter()
            .map(|(layer, (trades, wins, total_pnl, total_volume))| LayerPerformance {
                layer,
                trades,
                wins,
                total_pnl: (total_pnl * 100.0).round() / 100.0,
                win_rate: if trades > 0 { (wins as f64 / trades as f64 * 1000.0).round() / 10.0 } else { 0.0 },
                avg_volume: if trades > 0 { (total_volume / trades as f64 * 10000.0).round() / 10000.0 } else { 0.0 },
                avg_profit: if trades > 0 { (total_pnl / trades as f64 * 100.0).round() / 100.0 } else { 0.0 },
            })
            .collect();

        result.sort_by_key(|l| l.layer);
        result
    }

    pub fn volume_analysis(&self, deals: &[Deal]) -> VolumeAnalysis {
        let closed: Vec<&Deal> = deals
            .iter()
            .filter(|d| d.entry.to_lowercase().contains("out") && d.profit != 0.0)
            .collect();

        if closed.is_empty() {
            return VolumeAnalysis {
                correlation_with_profit: 0.0,
                by_volume_bucket: vec![],
            };
        }

        // Calculate correlation
        let n = closed.len() as f64;
        let sum_x: f64 = closed.iter().map(|d| d.volume).sum();
        let sum_y: f64 = closed.iter().map(|d| d.profit).sum();
        let sum_xy: f64 = closed.iter().map(|d| d.volume * d.profit).sum();
        let sum_x2: f64 = closed.iter().map(|d| d.volume * d.volume).sum();
        let sum_y2: f64 = closed.iter().map(|d| d.profit * d.profit).sum();

        let correlation = if n > 1.0 {
            let numerator = n * sum_xy - sum_x * sum_y;
            let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
            if denominator > 0.0 { numerator / denominator } else { 0.0 }
        } else {
            0.0
        };

        // Create volume buckets
        let bucket_defs = [
            (0.0, 0.1, "0.0-0.1 lots"),
            (0.1, 0.5, "0.1-0.5 lots"),
            (0.5, 1.0, "0.5-1.0 lots"),
            (1.0, 2.0, "1.0-2.0 lots"),
            (2.0, 5.0, "2.0-5.0 lots"),
            (5.0, 999.0, "5.0+ lots"),
        ];

        let by_volume_bucket: Vec<VolumeBucket> = bucket_defs
            .iter()
            .map(|(min, max, range)| {
                let bucket_deals: Vec<&Deal> = closed
                    .iter()
                    .filter(|d| d.volume >= *min && d.volume < *max)
                    .cloned()
                    .collect();
                let trades = bucket_deals.len() as i32;
                let total_pnl: f64 = bucket_deals.iter().map(|d| d.profit).sum();
                let avg_profit = if trades > 0 { total_pnl / trades as f64 } else { 0.0 };

                VolumeBucket {
                    volume_range: range.to_string(),
                    min_volume: *min,
                    max_volume: *max,
                    trades,
                    avg_profit: (avg_profit * 100.0).round() / 100.0,
                    total_pnl: (total_pnl * 100.0).round() / 100.0,
                }
            })
            .collect();

        VolumeAnalysis {
            correlation_with_profit: (correlation * 1000.0).round() / 1000.0,
            by_volume_bucket,
        }
    }

    pub fn cost_analysis(&self, deals: &[Deal]) -> CostAnalysis {
        let total_commission: f64 = deals.iter().map(|d| d.commission.abs()).sum();
        let total_swap: f64 = deals.iter().map(|d| d.swap.abs()).sum();
        let gross_profit: f64 = deals.iter().map(|d| d.profit).filter(|p| *p > 0.0).sum();
        let trade_count = deals.iter().filter(|d| d.entry.to_lowercase().contains("out")).count() as f64;

        let commission_pct = if gross_profit > 0.0 { (total_commission / gross_profit * 10000.0).round() / 100.0 } else { 0.0 };
        let swap_pct = if gross_profit > 0.0 { (total_swap / gross_profit * 10000.0).round() / 100.0 } else { 0.0 };

        // Calculate what win rate would be without costs
        let wins_before_costs = deals
            .iter()
            .filter(|d| {
                let profit_before_costs = d.profit + d.commission.abs() + d.swap.abs();
                d.entry.to_lowercase().contains("out") && profit_before_costs > 0.0
            })
            .count() as f64;
        let total_closed = deals.iter().filter(|d| d.entry.to_lowercase().contains("out")).count() as f64;
        let win_rate_before_costs = if total_closed > 0.0 { wins_before_costs / total_closed * 100.0 } else { 0.0 };

        let current_wins = deals.iter().filter(|d| d.entry.to_lowercase().contains("out") && d.profit > 0.0).count() as f64;
        let current_win_rate = if total_closed > 0.0 { current_wins / total_closed * 100.0 } else { 0.0 };

        CostAnalysis {
            total_commission: (total_commission * 100.0).round() / 100.0,
            total_swap: (total_swap * 100.0).round() / 100.0,
            commission_pct_of_profit: commission_pct,
            swap_pct_of_profit: swap_pct,
            avg_commission_per_trade: if trade_count > 0.0 { (total_commission / trade_count * 100.0).round() / 100.0 } else { 0.0 },
            avg_swap_per_trade: if trade_count > 0.0 { (total_swap / trade_count * 100.0).round() / 100.0 } else { 0.0 },
            net_profit_before_costs: (gross_profit * 100.0).round() / 100.0,
            cost_impact_on_win_rate: (win_rate_before_costs - current_win_rate * 100.0).round() / 100.0,
        }
    }

    pub fn efficiency_analysis(&self, deals: &[Deal], _metrics: &Metrics) -> EfficiencyAnalysis {
        let closed: Vec<&Deal> = deals
            .iter()
            .filter(|d| d.entry.to_lowercase().contains("out") && d.profit != 0.0)
            .collect();

        if closed.is_empty() {
            return EfficiencyAnalysis {
                profit_per_hour: 0.0,
                profit_per_day: 0.0,
                profit_per_trade_hour: 0.0,
                avg_trade_duration_hours: 0.0,
                annualized_return_pct: 0.0,
                trades_per_day: 0.0,
            };
        }

        let total_profit: f64 = closed.iter().map(|d| d.profit).sum();
        let total_trades = closed.len() as f64;

        // Calculate total hold time
        let mut total_hold_minutes = 0.0;
        let mut open_pos: HashMap<String, DateTime<chrono::Utc>> = HashMap::new();

        for deal in deals {
            let entry = deal.entry.to_lowercase();
            if let Some(dt) = Self::parse_datetime(&deal.time) {
                if entry.contains("in") && !entry.contains("out") {
                    open_pos.insert(deal.order.clone(), dt);
                } else if entry.contains("out") && deal.profit != 0.0 {
                    if let Some(in_time) = open_pos.remove(&deal.order) {
                        let hold_minutes = (dt - in_time).num_seconds() as f64 / 60.0;
                        if hold_minutes > 0.0 {
                            total_hold_minutes += hold_minutes;
                        }
                    }
                }
            }
        }

        let total_hold_hours = total_hold_minutes / 60.0;
        let avg_trade_duration = if total_trades > 0.0 { total_hold_minutes / total_trades / 60.0 } else { 0.0 };

        // Get date range
        let dates: Vec<DateTime<chrono::Utc>> = deals
            .iter()
            .filter_map(|d| Self::parse_datetime(&d.time))
            .collect();

        let total_days = if dates.len() >= 2 {
            let min_date = dates.iter().min().unwrap();
            let max_date = dates.iter().max().unwrap();
            (*max_date - *min_date).num_days().max(1) as f64
        } else {
            1.0
        };

        // Use a default deposit of 10000 for annualized calculation
        // In real scenarios, this should come from the report
        let deposit = 10000.0;
        let annualized = if total_days > 0.0 && deposit > 0.0 {
            let daily_return = total_profit / deposit / total_days;
            ((1.0 + daily_return).powf(365.0) - 1.0) * 100.0
        } else {
            0.0
        };

        EfficiencyAnalysis {
            profit_per_hour: if total_hold_hours > 0.0 { (total_profit / total_hold_hours * 100.0).round() / 100.0 } else { 0.0 },
            profit_per_day: (total_profit / total_days * 100.0).round() / 100.0,
            profit_per_trade_hour: if total_hold_hours > 0.0 { (total_profit / total_hold_hours / total_trades * 100.0).round() / 100.0 } else { 0.0 },
            avg_trade_duration_hours: (avg_trade_duration * 10.0).round() / 10.0,
            annualized_return_pct: (annualized * 10.0).round() / 10.0,
            trades_per_day: (total_trades / total_days * 10.0).round() / 10.0,
        }
    }
}
