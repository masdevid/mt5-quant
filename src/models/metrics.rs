use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metrics {
    pub net_profit: f64,
    pub profit_factor: f64,
    pub max_dd_pct: f64,
    pub sharpe_ratio: f64,
    pub total_trades: i32,
    pub recovery_factor: f64,
    pub win_rate_pct: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,
}

impl Metrics {
    pub fn from_html(text: &str) -> Option<Self> {
        let mut m = Metrics::default();

        let patterns = [
            ("net_profit", r"Net\s+Profit[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
            ("profit_factor", r"Profit\s+Factor[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
            ("max_dd_pct", r"Equity\s+Drawdown\s+Maximal[^<]*</td>\s*<td[^>]*>\s*<b>[^(]*\(([\d.,]+)%\)"),
            ("sharpe_ratio", r"Sharpe\s+Ratio[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
            ("total_trades", r"Total\s+Trades[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
            ("recovery_factor", r"Recovery\s+Factor[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
            ("win_rate_pct", r"Profit\s+Trades\s+\(%[^<]*</td>\s*<td[^>]*>\s*<b>[^(]*\(([\d.,]+)%\)"),
            ("gross_profit", r"Gross\s+Profit[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
            ("gross_loss", r"Gross\s+Loss[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>"),
        ];

        for (key, pattern) in &patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if let Some(captures) = regex.captures(text) {
                    if let Some(val_str) = captures.get(1) {
                        let val = val_str.as_str()
                            .replace(' ', "")
                            .replace(',', "");
                        
                        match *key {
                            "net_profit" => m.net_profit = val.parse().unwrap_or(0.0),
                            "profit_factor" => m.profit_factor = val.parse().unwrap_or(0.0),
                            "max_dd_pct" => m.max_dd_pct = val.parse().unwrap_or(0.0),
                            "sharpe_ratio" => m.sharpe_ratio = val.parse().unwrap_or(0.0),
                            "total_trades" => m.total_trades = val.parse().unwrap_or(0) as i32,
                            "recovery_factor" => m.recovery_factor = val.parse().unwrap_or(0.0),
                            "win_rate_pct" => m.win_rate_pct = val.parse().unwrap_or(0.0),
                            "gross_profit" => m.gross_profit = val.parse().unwrap_or(0.0),
                            "gross_loss" => m.gross_loss = val.parse().unwrap_or(0.0),
                            _ => {}
                        }
                    }
                }
            }
        }

        if m.total_trades > 0 {
            Some(m)
        } else {
            None
        }
    }
}
