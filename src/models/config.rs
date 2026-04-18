use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub wine_executable: Option<String>,
    pub terminal_dir: Option<String>,
    pub experts_dir: Option<String>,
    pub indicators_dir: Option<String>,
    pub scripts_dir: Option<String>,
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
            indicators_dir: None,
            scripts_dir: None,
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
        let config_path = Self::writable_config_path();

        if config_path.exists() {
            return Self::parse_file(&config_path);
        }

        // No config found — auto-discover and persist.
        let discovered = Self::auto_discover();
        if let Err(e) = discovered.save() {
            tracing::warn!("Could not save auto-discovered config: {}", e);
        }
        Ok(discovered)
    }

    /// The canonical writable config location: $MT5_MCP_HOME/config/mt5-quant.yaml
    /// or ~/.config/mt5-quant/config/mt5-quant.yaml.
    pub fn writable_config_path() -> PathBuf {
        if let Ok(home) = std::env::var("MT5_MCP_HOME") {
            return Path::new(&home).join("config").join("mt5-quant.yaml");
        }
        Self::installation_dir().join("config").join("mt5-quant.yaml")
    }

    // ── Auto-discovery ────────────────────────────────────────────────────────

    pub fn auto_discover() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut cfg = Config::default();

        // 1. Find Wine executable -------------------------------------------
        cfg.wine_executable = Self::find_wine(&home);

        // 2. Find MT5 terminal directory ------------------------------------
        if let Some(mt5_dir) = Self::find_mt5_dir(&home) {
            cfg.experts_dir = Some(
                mt5_dir.join("MQL5").join("Experts")
                    .to_string_lossy().to_string(),
            );
            cfg.indicators_dir = Some(
                mt5_dir.join("MQL5").join("Indicators")
                    .to_string_lossy().to_string(),
            );
            cfg.scripts_dir = Some(
                mt5_dir.join("MQL5").join("Scripts")
                    .to_string_lossy().to_string(),
            );
            cfg.tester_profiles_dir = Some(
                mt5_dir.join("MQL5").join("Profiles").join("Tester")
                    .to_string_lossy().to_string(),
            );
            cfg.tester_cache_dir = Some(
                mt5_dir.join("Tester")
                    .to_string_lossy().to_string(),
            );
            cfg.terminal_dir = Some(mt5_dir.to_string_lossy().to_string());
        }

        // 3. Display mode ---------------------------------------------------
        cfg.display_mode = Some(Self::detect_display_mode());

        // 4. Sensible backtest defaults ------------------------------------
        cfg.backtest_symbol   = Some("XAUUSD".into());
        cfg.backtest_deposit  = Some(10000);
        cfg.backtest_currency = Some("USD".into());
        cfg.backtest_leverage = Some(500);
        cfg.backtest_model    = Some(0);
        cfg.backtest_timeframe = Some("M5".into());
        cfg.backtest_timeout  = Some(900);
        cfg.opt_log_dir       = Some("/tmp".into());
        cfg.opt_min_agents    = Some(1);

        cfg
    }

    fn find_wine(home: &Path) -> Option<String> {
        let candidates: &[PathBuf] = &[
            // macOS: bundled with the official MT5 app
            PathBuf::from("/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"),
            // macOS: CrossOver
            home.join("Applications/CrossOver.app/Contents/SharedSupport/CrossOver/wine/bin/wine64"),
            // macOS: Homebrew (Apple Silicon)
            PathBuf::from("/opt/homebrew/bin/wine64"),
            PathBuf::from("/opt/homebrew/bin/wine"),
            // macOS: Homebrew (Intel)
            PathBuf::from("/usr/local/bin/wine64"),
            PathBuf::from("/usr/local/bin/wine"),
            // Linux
            PathBuf::from("/usr/bin/wine64"),
            PathBuf::from("/usr/bin/wine"),
        ];
        candidates.iter()
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
    }

    fn find_mt5_dir(home: &Path) -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = vec![
            // macOS: official MT5 app Wine prefix
            home.join("Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5"),
            // Linux / macOS Homebrew Wine
            home.join(".wine/drive_c/Program Files/MetaTrader 5"),
        ];

        // macOS CrossOver bottles: scan all bottles for an MT5 install
        let bottles_root = home.join("Library/Application Support/CrossOver/Bottles");
        if bottles_root.is_dir() {
            if let Ok(bottles) = fs::read_dir(&bottles_root) {
                for bottle in bottles.filter_map(|e| e.ok()) {
                    let mt5 = bottle.path()
                        .join("drive_c/Program Files/MetaTrader 5");
                    candidates.push(mt5);
                }
            }
        }

        candidates.into_iter().find(|p| p.is_dir())
    }

    fn detect_display_mode() -> String {
        // On macOS the MT5 native app handles display via its bundled Wine —
        // no Xvfb needed.
        if cfg!(target_os = "macos") {
            return "gui".into();
        }
        // Linux: use headless (Xvfb) when no X display is available.
        if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok() {
            "gui".into()
        } else {
            "headless".into()
        }
    }

    // ── Persistence ──────────────────────────────────────────────────────────

    pub fn save(&self) -> Result<()> {
        let path = Self::writable_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let none = || "~".to_string();
        let s = |v: &Option<String>| v.clone().unwrap_or_else(none);
        let u = |v: Option<u32>| v.map(|n| n.to_string()).unwrap_or_else(none);

        let content = format!(
            "# mt5-quant configuration — auto-generated on first run\n\
             # Edit freely; the server will not overwrite an existing file.\n\
             \n\
             wine_executable: {wine}\n\
             terminal_dir: {term}\n\
             experts_dir: {exp}\n\
             tester_profiles_dir: {prof}\n\
             tester_cache_dir: {cache}\n\
             display_mode: {disp}\n\
             \n\
             backtest_symbol: {sym}\n\
             backtest_deposit: {dep}\n\
             backtest_currency: {cur}\n\
             backtest_leverage: {lev}\n\
             backtest_model: {mdl}\n\
             backtest_timeframe: {tf}\n\
             backtest_timeout: {to}\n\
             \n\
             opt_log_dir: {opt_log}\n\
             opt_min_agents: {opt_agents}\n",
            wine      = s(&self.wine_executable),
            term      = s(&self.terminal_dir),
            exp       = s(&self.experts_dir),
            prof      = s(&self.tester_profiles_dir),
            cache     = s(&self.tester_cache_dir),
            disp      = s(&self.display_mode),
            sym       = s(&self.backtest_symbol),
            dep       = u(self.backtest_deposit),
            cur       = s(&self.backtest_currency),
            lev       = u(self.backtest_leverage),
            mdl       = u(self.backtest_model),
            tf        = s(&self.backtest_timeframe),
            to        = u(self.backtest_timeout),
            opt_log   = s(&self.opt_log_dir),
            opt_agents = u(self.opt_min_agents),
        );

        fs::write(&path, content)?;
        tracing::info!("Config written to {}", path.display());
        Ok(())
    }

    // ── Parsing ───────────────────────────────────────────────────────────────

    fn parse_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut map: HashMap<String, String> = HashMap::new();

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
                    map.insert(key, value);
                }
            }
        }

        Ok(Config {
            wine_executable:      map.get("wine_executable").cloned(),
            terminal_dir:         map.get("terminal_dir").cloned(),
            experts_dir:          map.get("experts_dir").cloned(),
            indicators_dir:       map.get("indicators_dir").cloned(),
            scripts_dir:          map.get("scripts_dir").cloned(),
            tester_profiles_dir:  map.get("tester_profiles_dir").cloned(),
            tester_cache_dir:     map.get("tester_cache_dir").cloned(),
            display_mode:         map.get("display_mode").cloned(),
            backtest_symbol:      map.get("backtest_symbol").cloned(),
            backtest_deposit:     map.get("backtest_deposit").and_then(|s| s.parse().ok()),
            backtest_currency:    map.get("backtest_currency").cloned(),
            backtest_leverage:    map.get("backtest_leverage").and_then(|s| s.parse().ok()),
            backtest_model:       map.get("backtest_model").and_then(|s| s.parse().ok()),
            backtest_timeframe:   map.get("backtest_timeframe").cloned(),
            backtest_timeout:     map.get("backtest_timeout").and_then(|s| s.parse().ok()),
            opt_log_dir:          map.get("opt_log_dir").cloned(),
            opt_min_agents:       map.get("opt_min_agents").and_then(|s| s.parse().ok()),
            reports_dir:          map.get("reports_dir").cloned(),
            backtest_login:       map.get("backtest_login").cloned(),
            backtest_server:      map.get("backtest_server").cloned(),
            project_dir:          map.get("project_dir").cloned(),
        })
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub fn get(&self, key: &str) -> String {
        match key {
            "wine_executable"    => self.wine_executable.clone().unwrap_or_default(),
            "terminal_dir"       => self.terminal_dir.clone().unwrap_or_default(),
            "experts_dir"        => self.experts_dir.clone().unwrap_or_default(),
            "tester_profiles_dir"=> self.tester_profiles_dir.clone().unwrap_or_default(),
            "tester_cache_dir"   => self.tester_cache_dir.clone().unwrap_or_default(),
            "display_mode"       => self.display_mode.clone().unwrap_or_else(|| "auto".to_string()),
            "backtest_symbol"    => self.backtest_symbol.clone().unwrap_or_default(),
            "backtest_deposit"   => self.backtest_deposit.unwrap_or(10000).to_string(),
            "backtest_currency"  => self.backtest_currency.clone().unwrap_or_else(|| "USD".to_string()),
            "backtest_leverage"  => self.backtest_leverage.unwrap_or(500).to_string(),
            "backtest_model"     => self.backtest_model.unwrap_or(0).to_string(),
            "backtest_timeframe" => self.backtest_timeframe.clone().unwrap_or_else(|| "M5".to_string()),
            "backtest_timeout"   => self.backtest_timeout.unwrap_or(900).to_string(),
            "opt_log_dir"        => self.opt_log_dir.clone().unwrap_or_else(|| "/tmp".to_string()),
            "opt_min_agents"     => self.opt_min_agents.unwrap_or(1).to_string(),
            "reports_dir"        => self.reports_dir.clone().unwrap_or_else(|| "reports".to_string()),
            "backtest_login"     => self.backtest_login.clone().unwrap_or_default(),
            "backtest_server"    => self.backtest_server.clone().unwrap_or_default(),
            "project_dir"        => self.project_dir.clone().unwrap_or_default(),
            _                    => String::new(),
        }
    }

    /// Root of the MCP installation: $MT5_MCP_HOME or ~/.config/mt5-quant
    pub fn installation_dir() -> PathBuf {
        if let Ok(home) = std::env::var("MT5_MCP_HOME") {
            return Path::new(&home).to_path_buf();
        }
        dirs::home_dir()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
            .join(".config")
            .join("mt5-quant")
    }

    /// Centralized report data directory (metadata + deals, no HTML).
    /// Always inside the MCP installation dir, never in the project.
    pub fn reports_dir(&self) -> PathBuf {
        if let Some(dir) = &self.reports_dir {
            let p = Path::new(dir);
            if p.is_absolute() {
                return p.to_path_buf();
            }
        }
        Self::installation_dir().join("reports")
    }

    /// Path to the SQLite report registry.
    pub fn db_path() -> PathBuf {
        Self::installation_dir().join("reports.db")
    }

    /// Temp directory for equity chart images, scoped per report.
    pub fn charts_temp_dir(report_id: &str) -> PathBuf {
        std::env::temp_dir()
            .join("mt5-quant")
            .join("charts")
            .join(report_id)
    }

    pub fn mt5_dir(&self) -> Option<PathBuf> {
        self.terminal_dir.as_ref().map(|d| Path::new(d).to_path_buf())
    }

    /// Scan Bases/*/history/ for symbol directories that contain at least one .hcc file.
    /// Returns deduplicated, sorted list of symbol names available for backtesting.
    pub fn discover_symbols(&self) -> Vec<String> {
        let mt5_dir = match self.mt5_dir() {
            Some(d) => d,
            None => return Vec::new(),
        };

        let bases_dir = mt5_dir.join("Bases");
        if !bases_dir.is_dir() {
            return Vec::new();
        }

        let mut symbols = std::collections::HashSet::new();

        // Bases/{server}/history/{symbol}/{year}.hcc
        if let Ok(servers) = fs::read_dir(&bases_dir) {
            for server in servers.filter_map(|e| e.ok()) {
                let history_dir = server.path().join("history");
                if !history_dir.is_dir() {
                    continue;
                }
                if let Ok(sym_entries) = fs::read_dir(&history_dir) {
                    for sym_entry in sym_entries.filter_map(|e| e.ok()) {
                        let sym_path = sym_entry.path();
                        if !sym_path.is_dir() {
                            continue;
                        }
                        // Only include if at least one .hcc file exists (has downloaded data)
                        let has_data = fs::read_dir(&sym_path)
                            .ok()
                            .map(|entries| {
                                entries.filter_map(|e| e.ok()).any(|e| {
                                    e.path().extension()
                                        .and_then(|x| x.to_str())
                                        .map(|x| x == "hcc")
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false);

                        if has_data {
                            if let Some(name) = sym_path.file_name().and_then(|n| n.to_str()) {
                                symbols.insert(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut sorted: Vec<String> = symbols.into_iter().collect();
        sorted.sort();
        sorted
    }
}
