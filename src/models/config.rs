use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub wine_executable: Option<String>,
    pub terminal_dir: Option<String>,
    pub experts_dir: Option<String>,
    pub tester_profiles_dir: Option<String>,
    pub tester_cache_dir: Option<String>,
    pub display_mode: Option<String>,
    pub backtest_symbol: Option<String>,
    pub backtest_deposit: Option<u32>,
    pub backtest_currency: Option<String>,
    pub backtest_leverage: Option<u32>,
    pub backtest_model: Option<u32>,
    pub backtest_timeframe: Option<String>,
    pub backtest_timeout: Option<u32>,
    pub opt_log_dir: Option<String>,
    pub opt_min_agents: Option<u32>,
    pub reports_dir: Option<String>,
    pub backtest_login: Option<String>,
    pub backtest_server: Option<String>,
    pub project_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wine_executable: None,
            terminal_dir: None,
            experts_dir: None,
            tester_profiles_dir: None,
            tester_cache_dir: None,
            display_mode: None,
            backtest_symbol: None,
            backtest_deposit: None,
            backtest_currency: None,
            backtest_leverage: None,
            backtest_model: None,
            backtest_timeframe: None,
            backtest_timeout: None,
            opt_log_dir: None,
            opt_min_agents: None,
            reports_dir: None,
            backtest_login: None,
            backtest_server: None,
            project_dir: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path();
        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&config_path)?;
        let mut config: HashMap<String, String> = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || !line.contains(':') {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();

                if !value.is_empty() && value != "null" && value != "~" {
                    config.insert(key, value);
                }
            }
        }

        Ok(Config {
            wine_executable: config.get("wine_executable").cloned(),
            terminal_dir: config.get("terminal_dir").cloned(),
            experts_dir: config.get("experts_dir").cloned(),
            tester_profiles_dir: config.get("tester_profiles_dir").cloned(),
            tester_cache_dir: config.get("tester_cache_dir").cloned(),
            display_mode: config.get("display_mode").cloned(),
            backtest_symbol: config.get("backtest_symbol").cloned(),
            backtest_deposit: config.get("backtest_deposit").and_then(|s| s.parse().ok()),
            backtest_currency: config.get("backtest_currency").cloned(),
            backtest_leverage: config.get("backtest_leverage").and_then(|s| s.parse().ok()),
            backtest_model: config.get("backtest_model").and_then(|s| s.parse().ok()),
            backtest_timeframe: config.get("backtest_timeframe").cloned(),
            backtest_timeout: config.get("backtest_timeout").and_then(|s| s.parse().ok()),
            opt_log_dir: config.get("opt_log_dir").cloned(),
            opt_min_agents: config.get("opt_min_agents").and_then(|s| s.parse().ok()),
            reports_dir: config.get("reports_dir").cloned(),
            backtest_login: config.get("backtest_login").cloned(),
            backtest_server: config.get("backtest_server").cloned(),
            project_dir: config.get("project_dir").cloned(),
        })
    }

    pub fn get_config_path() -> std::path::PathBuf {
        if let Ok(home) = std::env::var("MT5_MCP_HOME") {
            Path::new(&home).join("config").join("mt5-quant.yaml")
        } else {
            let base_path = dirs::home_dir()
                .unwrap_or_else(|| Path::new(".").to_path_buf())
                .join(".config")
                .join("mt5-quant");

            if base_path.join("config").join("mt5-quant.yaml").exists() {
                base_path.join("config").join("mt5-quant.yaml")
            } else {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("config")
                    .join("mt5-quant.yaml")
            }
        }
    }

    pub fn get(&self, key: &str) -> String {
        match key {
            "wine_executable" => self.wine_executable.clone().unwrap_or_default(),
            "terminal_dir" => self.terminal_dir.clone().unwrap_or_default(),
            "experts_dir" => self.experts_dir.clone().unwrap_or_default(),
            "tester_profiles_dir" => self.tester_profiles_dir.clone().unwrap_or_default(),
            "tester_cache_dir" => self.tester_cache_dir.clone().unwrap_or_default(),
            "display_mode" => self.display_mode.clone().unwrap_or_else(|| "auto".to_string()),
            "backtest_symbol" => self.backtest_symbol.clone().unwrap_or_default(),
            "backtest_deposit" => self.backtest_deposit.unwrap_or(10000).to_string(),
            "backtest_currency" => self.backtest_currency.clone().unwrap_or_else(|| "USD".to_string()),
            "backtest_leverage" => self.backtest_leverage.unwrap_or(500).to_string(),
            "backtest_model" => self.backtest_model.unwrap_or(0).to_string(),
            "backtest_timeframe" => self.backtest_timeframe.clone().unwrap_or_else(|| "M5".to_string()),
            "backtest_timeout" => self.backtest_timeout.unwrap_or(900).to_string(),
            "opt_log_dir" => self.opt_log_dir.clone().unwrap_or_else(|| "/tmp".to_string()),
            "opt_min_agents" => self.opt_min_agents.unwrap_or(1).to_string(),
            "reports_dir" => self.reports_dir.clone().unwrap_or_else(|| "reports".to_string()),
            "backtest_login" => self.backtest_login.clone().unwrap_or_default(),
            "backtest_server" => self.backtest_server.clone().unwrap_or_default(),
            "project_dir" => self.project_dir.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }

    pub fn reports_dir(&self) -> std::path::PathBuf {
        Path::new(&self.get("reports_dir")).to_path_buf()
    }

    pub fn mt5_dir(&self) -> Option<std::path::PathBuf> {
        self.terminal_dir.as_ref().map(|d| Path::new(d).to_path_buf())
    }
}
