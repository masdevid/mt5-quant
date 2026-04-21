use anyhow::{anyhow, Result};
use chrono;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::time::{sleep, Duration};

use crate::analytics::{DealAnalyzer, ReportExtractor};
use crate::compile::MqlCompiler;
use crate::models::config::Config;
use crate::models::report::{PipelineMetadata, FilePaths, BacktestJob};
use crate::storage::{ReportDb, ReportEntry};

pub struct BacktestPipeline {
    config: Config,
    compiler: MqlCompiler,
    extractor: ReportExtractor,
    analyzer: DealAnalyzer,
}

pub struct BacktestParams {
    pub expert: String,
    pub symbol: String,
    pub from_date: String,
    pub to_date: String,
    pub timeframe: String,
    pub deposit: u32,
    pub model: u8,
    pub leverage: u32,
    pub set_file: Option<String>,
    pub skip_compile: bool,
    pub skip_clean: bool,
    pub skip_analyze: bool,
    #[allow(dead_code)]
    pub deep_analyze: bool,
    pub shutdown: bool,
    #[allow(dead_code)]
    pub kill_existing: bool,
    pub timeout: u64,
    pub gui: bool,
}

pub struct PipelineResult {
    pub success: bool,
    pub report_dir: PathBuf,
    pub duration_seconds: i64,
    pub message: String,
}

impl BacktestPipeline {
    pub fn new(config: Config) -> Self {
        let compiler = MqlCompiler::new(config.clone());
        let extractor = ReportExtractor::new();
        let analyzer = DealAnalyzer::new();
        
        Self {
            config,
            compiler,
            extractor,
            analyzer,
        }
    }

    pub async fn run(&self, params: BacktestParams) -> Result<PipelineResult> {
        let start_time = chrono::Utc::now();
        let report_id = self.generate_report_id(&params);
        let report_dir = self.config.reports_dir().join(&report_id);

        fs::create_dir_all(&report_dir)?;

        let progress_log = report_dir.join("progress.log");
        self.log_progress(&progress_log, "START").await;

        if !params.skip_compile {
            self.log_progress(&progress_log, "COMPILE").await;
            self.compile_ea(&params.expert, params.timeout).await?;
        }

        if !params.skip_clean {
            self.log_progress(&progress_log, "CLEAN").await;
            self.clean_cache(&params.expert).await?;
        }

        self.log_progress(&progress_log, "BACKTEST").await;
        let report_path = self.run_backtest(&params, &report_id).await?;

        self.log_progress(&progress_log, "EXTRACT").await;
        let extraction = self.extractor.extract(
            &report_path.to_string_lossy(),
            &report_dir.to_string_lossy(),
        )?;

        // Handle case where EA didn't trade - no deals generated
        if extraction.deals.is_empty() {
            tracing::warn!("Backtest completed but no deals were generated - EA did not trade during this period");
            let warning_path = report_dir.join("NO_TRADES_WARNING.txt");
            let _ = fs::write(&warning_path, "Warning: No deals were generated during this backtest.\nThe EA did not execute any trades during the specified date range.\n");
        }

        // Move equity chart images to OS temp dir, then delete the HTML report.
        let charts_dir = self.relocate_charts(&report_path, &report_id).await;
        let _ = fs::remove_file(&report_path);

        // Snapshot the set file alongside the extracted data.
        let set_snapshot = self.snapshot_set_file(&params, &report_dir).await;

        if !params.skip_analyze {
            self.log_progress(&progress_log, "ANALYZE").await;
            let analysis = self.analyzer.analyze(&extraction.deals, &extraction.metrics);

            let analysis_path = report_dir.join("analysis.json");
            fs::write(&analysis_path, serde_json::to_string_pretty(&analysis)?)?;
        }

        self.log_progress(&progress_log, "DONE").await;

        let duration = (chrono::Utc::now() - start_time).num_seconds();
        self.save_metadata(&params, &report_dir, duration, extraction.deals.is_empty()).await?;

        // Register in the SQLite report registry.
        self.register_in_db(
            &report_id,
            &params,
            &report_dir,
            charts_dir.as_deref(),
            set_snapshot.as_deref(),
            &extraction.metrics,
            duration,
        )
        .await;

        let message = if extraction.deals.is_empty() {
            "Backtest completed successfully, but EA did not execute any trades during this period".to_string()
        } else {
            "Backtest completed successfully".to_string()
        };

        Ok(PipelineResult {
            success: true,
            report_dir,
            duration_seconds: duration,
            message,
        })
    }

    /// Launch backtest in fire-and-forget mode: compile, clean, launch MT5, return immediately.
    /// Returns a BacktestJob that can be used with get_backtest_status to poll for completion.
    pub async fn launch_backtest(&self, params: BacktestParams) -> Result<BacktestJob> {
        let _start_time = chrono::Utc::now();
        let report_id = self.generate_report_id(&params);
        let report_dir = self.config.reports_dir().join(&report_id);

        fs::create_dir_all(&report_dir)?;

        let progress_log = report_dir.join("progress.log");
        self.log_progress(&progress_log, "START").await;

        if !params.skip_compile {
            self.log_progress(&progress_log, "COMPILE").await;
            self.compile_ea(&params.expert, params.timeout).await?;
        }

        if !params.skip_clean {
            self.log_progress(&progress_log, "CLEAN").await;
            self.clean_cache(&params.expert).await?;
        }

        self.log_progress(&progress_log, "BACKTEST").await;
        
        // Get MT5 paths
        let mt5_dir = self.config.mt5_dir()
            .ok_or_else(|| anyhow!("MT5 directory not configured"))?;
        let wine_exe = self.config.wine_executable.as_ref()
            .ok_or_else(|| anyhow!("wine_executable not configured"))?;
        let wine_prefix = mt5_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow!("Could not determine Wine prefix from terminal_dir"))?;
        let reports_dir = mt5_dir.join("reports");
        fs::create_dir_all(&reports_dir)?;

        // Write config files
        let ini_content = self.build_backtest_ini(&params, &report_id)?;
        let config_host = wine_prefix.join("drive_c").join("backtest_config.ini");
        fs::write(&config_host, ini_content.as_bytes())?;
        self.update_terminal_ini(&params, &report_id)?;

        // Kill any running MT5
        self.kill_mt5().await?;

        // Launch MT5 (fire and forget)
        let mut cmd = self.build_wine_launch(wine_exe, &wine_prefix)?;
        let child = cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let pid = child.id();
        tracing::info!("MT5 launched with PID {:?} for backtest {}", pid, report_id);

        // Create and save the job tracking file
        let expected_report = reports_dir.join(format!("{}.htm", report_id));
        let job = BacktestJob::new(
            report_id.clone(),
            report_dir.to_string_lossy().to_string(),
            params.expert.clone(),
            params.symbol.clone(),
            params.timeframe.clone(),
            expected_report.to_string_lossy().to_string(),
            params.timeout,
        );

        // Save job info for polling
        let job_path = report_dir.join("job.json");
        fs::write(&job_path, serde_json::to_string_pretty(&job)?)?;

        // Save initial metadata
        self.save_metadata(&params, &report_dir, 0, false).await?;

        // Register in DB as "running"
        let db = ReportDb::new(&Config::db_path());
        if let Err(e) = db.init() {
            tracing::warn!("Failed to init report DB: {}", e);
        }

        Ok(job)
    }

    /// Move equity chart images (*.png, *.gif) from MT5's reports dir to OS temp,
    /// returning the temp path if any images were found.
    async fn relocate_charts(&self, html_path: &Path, report_id: &str) -> Option<PathBuf> {
        let reports_dir = html_path.parent()?;
        let charts_dir = Config::charts_temp_dir(report_id);
        let image_exts = ["png", "gif", "jpg", "jpeg"];

        let entries = fs::read_dir(reports_dir).ok()?;
        let mut found = false;

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_string();

            let is_chart = name.starts_with(report_id)
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| image_exts.contains(&e))
                    .unwrap_or(false);

            if is_chart {
                if !found {
                    if fs::create_dir_all(&charts_dir).is_err() {
                        return None;
                    }
                }
                let dest = charts_dir.join(entry.file_name());
                let _ = fs::rename(&path, &dest);
                found = true;
            }
        }

        if found { Some(charts_dir) } else { None }
    }

    /// Copy the set file into the report dir as set_snapshot.set.
    async fn snapshot_set_file(
        &self,
        params: &BacktestParams,
        report_dir: &Path,
    ) -> Option<PathBuf> {
        let set_src = params.set_file.as_ref()?;
        let src_path = Path::new(set_src);
        if !src_path.exists() {
            return None;
        }
        let dest = report_dir.join("set_snapshot.set");
        fs::copy(src_path, &dest).ok()?;
        Some(dest)
    }

    async fn register_in_db(
        &self,
        report_id: &str,
        params: &BacktestParams,
        report_dir: &Path,
        charts_dir: Option<&Path>,
        set_snapshot: Option<&Path>,
        metrics: &crate::models::metrics::Metrics,
        duration: i64,
    ) {
        let db = ReportDb::new(&Config::db_path());
        if let Err(e) = db.init() {
            tracing::warn!("Failed to init report DB: {}", e);
            return;
        }

        let entry = ReportEntry {
            id: report_id.to_string(),
            expert: params.expert.clone(),
            symbol: params.symbol.clone(),
            timeframe: params.timeframe.clone(),
            model: params.model as i64,
            from_date: params.from_date.clone(),
            to_date: params.to_date.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            set_file_original: params.set_file.clone(),
            set_snapshot_path: set_snapshot.map(|p| p.to_string_lossy().to_string()),
            report_dir: report_dir.to_string_lossy().to_string(),
            charts_dir: charts_dir.map(|p| p.to_string_lossy().to_string()),
            net_profit: Some(metrics.net_profit),
            profit_factor: Some(metrics.profit_factor),
            max_dd_pct: Some(metrics.max_dd_pct),
            sharpe_ratio: Some(metrics.sharpe_ratio),
            total_trades: Some(metrics.total_trades as i64),
            win_rate_pct: Some(metrics.win_rate_pct),
            recovery_factor: Some(metrics.recovery_factor),
            deposit: Some(params.deposit as f64),
            currency: self.config.backtest_currency.clone(),
            leverage: Some(params.leverage as i64),
            duration_seconds: Some(duration),
            tags: Vec::new(),
            notes: None,
            verdict: None,
        };

        if let Err(e) = db.insert(&entry) {
            tracing::warn!("Failed to register report in DB: {}", e);
        }
    }

    async fn compile_ea(&self, expert: &str, timeout_secs: u64) -> Result<()> {
        let mut search_paths = vec![
            PathBuf::from(&self.config.get("project_dir")).join("src/experts").join(format!("{}.mq5", expert)),
            PathBuf::from(&self.config.get("project_dir")).join("src").join(format!("{}.mq5", expert)),
            PathBuf::from(&self.config.get("project_dir")).join(format!("{}.mq5", expert)),
            PathBuf::from("src/experts").join(format!("{}.mq5", expert)),
            PathBuf::from("src").join(format!("{}.mq5", expert)),
            PathBuf::from(format!("{}.mq5", expert)),
        ];
        // Also search in MT5 Experts dir: Experts/{expert}/{expert}.mq5 and Experts/{expert}.mq5
        if let Some(experts_dir) = &self.config.experts_dir {
            search_paths.push(PathBuf::from(experts_dir).join(expert).join(format!("{}.mq5", expert)));
            search_paths.push(PathBuf::from(experts_dir).join(format!("{}.mq5", expert)));
        }

        let source_path = search_paths
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| anyhow!("Cannot find {}.mq5 — searched project_dir and MT5 Experts dir", expert))?;

        let timeout = std::time::Duration::from_secs(timeout_secs.min(300)); // Max 5 min for compile
        let result = self.compiler.compile_with_timeout(&source_path.to_string_lossy(), timeout).await?;
        
        if !result.success {
            return Err(anyhow!(
                "Compilation failed: {}",
                result.errors.join("; ")
            ));
        }

        Ok(())
    }

    async fn clean_cache(&self, expert: &str) -> Result<()> {
        if let Some(cache_dir) = &self.config.tester_cache_dir {
            let cache_path = Path::new(cache_dir);
            if cache_path.exists() {
                for entry in walkdir::WalkDir::new(cache_path) {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.extension().map(|e| e == "tst").unwrap_or(false) {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }
        }

        if let Some(tester_dir) = &self.config.tester_profiles_dir {
            let cached_set = Path::new(tester_dir).join(format!("{}.set", expert));
            if cached_set.exists() {
                let _ = fs::remove_file(&cached_set);
            }
        }

        self.reset_terminal_ini().await?;

        Ok(())
    }

    async fn reset_terminal_ini(&self) -> Result<()> {
        let mt5_dir = self.config.mt5_dir()
            .ok_or_else(|| anyhow!("MT5 directory not configured"))?;
        
        let terminal_ini = mt5_dir.join("config").join("terminal.ini");
        if !terminal_ini.exists() {
            return Ok(());
        }

        let content = fs::read(&terminal_ini)?;
        
        let (text, encoding) = if content.starts_with(&[0xFF, 0xFE]) || content.starts_with(&[0xFE, 0xFF]) {
            let text = String::from_utf16_lossy(
                content.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>()
                    .as_slice()
            );
            (text, "utf-16")
        } else {
            (String::from_utf8_lossy(&content).to_string(), "utf-8")
        };

        let updated = text
            .replace("OptMode=-1", "OptMode=0")
            .replace("LastOptimization=1", "");

        let output = if encoding == "utf-16" {
            let utf16: Vec<u16> = updated.encode_utf16().collect();
            let bytes: Vec<u8> = utf16.iter()
                .flat_map(|&c| c.to_le_bytes())
                .collect();
            bytes
        } else {
            updated.into_bytes()
        };

        fs::write(&terminal_ini, output)?;
        
        Ok(())
    }

    async fn run_backtest(&self, params: &BacktestParams, report_id: &str) -> Result<PathBuf> {
        let mt5_dir = self.config.mt5_dir()
            .ok_or_else(|| anyhow!("MT5 directory not configured"))?;

        let wine_exe = self.config.wine_executable.as_ref()
            .ok_or_else(|| anyhow!("wine_executable not configured"))?;

        // mt5_dir = {prefix}/drive_c/Program Files/MetaTrader 5
        // WINEPREFIX = three levels up
        let wine_prefix = mt5_dir
            .parent()                 // .../drive_c/Program Files
            .and_then(|p| p.parent()) // .../drive_c
            .and_then(|p| p.parent()) // .../<prefix>
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow!("Could not determine Wine prefix from terminal_dir"))?;

        let reports_dir = mt5_dir.join("reports");
        fs::create_dir_all(&reports_dir)?;

        // Write backtest_config.ini (used by wine64 shell-script launch via /config:)
        // and also patch terminal.ini so the Strategy Tester panel shows the right
        // settings if the user opens MT5 manually.
        let ini_content = self.build_backtest_ini(params, report_id)?;
        // Write to drive_c root (C:\backtest_config.ini) — no spaces in path avoids
        // Wine argument-quoting issues when MT5 parses the /config: value.
        let config_host = wine_prefix.join("drive_c").join("backtest_config.ini");
        fs::write(&config_host, ini_content.as_bytes())?;
        self.update_terminal_ini(params, report_id)?;

        // Always kill any running MT5 — Wine allows only one instance per prefix.
        self.kill_mt5().await?;

        // Record launch time before sleeping so find_newest_report doesn't miss
        // reports written during the startup wait.
        let poll_start = std::time::SystemTime::now();
        let launch_instant = tokio::time::Instant::now();

        // Build the launch command, adapting for the Wine runtime in use.
        let mut cmd = self.build_wine_launch(wine_exe, &wine_prefix)?;
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        // Give MT5 time to fully initialize before polling.
        // MT5 app startup (Wine init + network auth + tester) typically takes 15–20 s.
        sleep(Duration::from_secs(20)).await;

        // Poll for the report file (MT5 writes it when the backtest completes).
        // Grace period: don't check process liveness for the first 30 s after launch —
        // MT5 may still be appearing in the process list while wineserver re-initializes.
        let grace_period = Duration::from_secs(30);
        let deadline = launch_instant + Duration::from_secs(params.timeout);

        loop {
            let elapsed = launch_instant.elapsed().as_secs();

            // 1. Check for the exact expected report filename.
            for ext in &[".htm", ".htm.xml", ".html"] {
                let candidate = reports_dir.join(format!("{}{}", report_id, ext));
                tracing::debug!("poll t+{}s: checking {}", elapsed, candidate.display());
                if candidate.exists() {
                    tracing::info!("poll t+{}s: found exact report {}", elapsed, candidate.display());
                    return Ok(candidate);
                }
            }

            // 2. Only check process liveness after the grace period — this prevents
            //    a false "not running" when the new instance is still starting up.
            let in_grace = launch_instant.elapsed() <= grace_period;
            let mt5_alive = Self::is_mt5_running();
            tracing::info!("poll t+{}s: in_grace={} mt5_alive={}", elapsed, in_grace, mt5_alive);

            if !in_grace && !mt5_alive {
                if let Some(path) = Self::find_newest_report(&reports_dir, poll_start) {
                    tracing::info!("poll: MT5 exited, found fallback report {}", path.display());
                    return Ok(path);
                }
                return Err(anyhow!(
                    "MT5 exited without producing a report. \
                     The backtest may have been stopped mid-way or failed to start."
                ));
            }

            if tokio::time::Instant::now() > deadline {
                return Err(anyhow!("Timeout: no report after {} seconds", params.timeout));
            }
            sleep(Duration::from_secs(2)).await;
        }
    }

    /// Write backtest parameters into terminal.ini [Tester] section so MT5 reads
    /// them on startup without needing a /config: command-line argument.
    fn update_terminal_ini(&self, params: &BacktestParams, report_id: &str) -> Result<()> {
        let mt5_dir = self.config.mt5_dir()
            .ok_or_else(|| anyhow!("MT5 directory not configured"))?;
        let terminal_ini = mt5_dir.join("config").join("terminal.ini");

        let raw = fs::read(&terminal_ini)
            .unwrap_or_default();
        let text = if raw.starts_with(&[0xFF, 0xFE]) {
            raw[2..].chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>()
                .iter()
                .map(|&c| char::from_u32(c as u32).unwrap_or('?'))
                .collect::<String>()
        } else {
            String::from_utf8_lossy(&raw).into_owned()
        };

        let period = match params.timeframe.as_str() {
            "M1"  => 1u32,  "M5"  => 5,  "M15" => 15, "M30" => 30,
            "H1"  => 60,    "H4"  => 240, "D1"  => 1440,
            _     => 5,
        };

        let from_ts = Self::date_str_to_unix(&params.from_date)?;
        let to_ts   = Self::date_str_to_unix(&params.to_date)?;

        let currency = self.config.backtest_currency.as_deref().unwrap_or("USD");
        let set_file_line = params.set_file.as_ref()
            .map(|p| format!("ExpertParameters={}\n", p))
            .unwrap_or_default();

        let updates: &[(&str, String)] = &[
            ("Expert",           self.resolve_expert_path(&params.expert)),
            ("Symbol",           params.symbol.clone()),
            ("Period",           period.to_string()),
            ("DateRange",        "3".into()),
            ("DateFrom",         from_ts.to_string()),
            ("DateTo",           to_ts.to_string()),
            ("Visualization",    "0".into()),
            ("Execution",        "10".into()),
            ("Currency",         currency.into()),
            ("Leverage",         params.leverage.to_string()),
            ("Deposit",          format!("{:.2}", params.deposit)),
            ("TicksMode",        params.model.to_string()),
            ("PipsCalculation",  "1".into()),
            ("OptMode",          "0".into()),
            ("Report",           format!("reports\\{}.htm", report_id)),
            ("ReplaceReport",    "1".into()),
            ("ShutdownTerminal", if params.shutdown { "1" } else { "0" }.into()),
        ];

        let updated = Self::patch_ini_section(&text, "Tester", updates)
            + &set_file_line;

        let bom_utf16: Vec<u8> = [0xFF, 0xFE].iter().copied()
            .chain(updated.encode_utf16().flat_map(|c| c.to_le_bytes()))
            .collect();
        fs::write(&terminal_ini, bom_utf16)?;
        tracing::info!("terminal.ini [Tester] updated for backtest {}", report_id);
        Ok(())
    }

    /// Parse "YYYY.MM.DD" and return a Unix timestamp (seconds since 1970-01-01 UTC).
    fn date_str_to_unix(date: &str) -> Result<i64> {
        let parts: Vec<u32> = date.split('.').filter_map(|p| p.parse().ok()).collect();
        if parts.len() != 3 {
            return Err(anyhow!("Invalid date format: {}", date));
        }
        let dt = chrono::NaiveDate::from_ymd_opt(parts[0] as i32, parts[1], parts[2])
            .ok_or_else(|| anyhow!("Invalid date: {}", date))?
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("Date conversion failed"))?;
        Ok(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).timestamp())
    }

    /// Replace or add key=value pairs in a named INI section.
    fn patch_ini_section(text: &str, section: &str, updates: &[(&str, String)]) -> String {
        let section_header = format!("[{}]", section);
        let mut result = String::with_capacity(text.len() + 256);
        let mut in_section = false;
        let mut pending: std::collections::HashMap<&str, &String> =
            updates.iter().map(|(k, v)| (*k, v)).collect();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == section_header {
                in_section = true;
                result.push_str(line);
                result.push('\n');
                continue;
            }
            if trimmed.starts_with('[') && in_section {
                // End of our section — flush any keys not yet written
                for (k, v) in &pending {
                    result.push_str(&format!("{}={}\n", k, v));
                }
                pending.clear();
                in_section = false;
            }
            if in_section {
                if let Some((key, _)) = trimmed.split_once('=') {
                    let key = key.trim();
                    if let Some(val) = pending.remove(key) {
                        result.push_str(&format!("{}={}\n", key, val));
                        continue;
                    }
                }
            }
            result.push_str(line);
            result.push('\n');
        }
        // Flush remaining keys if section was at end of file
        if in_section {
            for (k, v) in &pending {
                result.push_str(&format!("{}={}\n", k, v));
            }
        }
        result
    }

    /// Build the OS-appropriate command to launch MT5 with the backtest config.
    ///
    /// - macOS MT5.app bundle: use `open -a "MetaTrader 5" --args /config:...`
    ///   The native launcher handles Wine env setup (DYLD vars are stripped by SIP
    ///   when set on child processes spawned from a Rust binary).
    /// - macOS CrossOver / Linux Wine: standard WINEPREFIX + wine64 direct invocation.
    fn build_wine_launch(&self, wine_exe: &str, wine_prefix: &Path) -> Result<Command> {
        if wine_exe.contains("MetaTrader 5.app") {
            // macOS MT5.app — the Swift launcher ignores --args so we can't pass
            // /config: via `open`. Instead, write a temp shell script that sets
            // DYLD_FALLBACK_LIBRARY_PATH and invokes wine64 with /config: directly.
            // Shell scripts bypass the SIP restriction that strips DYLD_* vars
            // when Rust spawns a codesigned binary as a direct child process.
            let wine_bin = Path::new(wine_exe);
            let wine_root = wine_bin
                .parent()                  // bin/
                .and_then(|p| p.parent())  // wine/
                .map(|p| p.to_path_buf())
                .ok_or_else(|| anyhow!("Cannot derive Wine root from wine_exe"))?;

            let ext_libs  = wine_root.join("lib").join("external");
            let wine_libs = wine_root.join("lib");
            let dyld = format!("{}:{}:/usr/lib:/usr/local/lib",
                ext_libs.display(), wine_libs.display());

            // Use host path for the exe; config at drive root to avoid spaces in path.
            let terminal_host = wine_prefix.join("drive_c")
                .join("Program Files").join("MetaTrader 5").join("terminal64.exe");
            let config_win = r"C:\backtest_config.ini";

            let script = format!(
                "#!/bin/sh\n\
                 export DYLD_FALLBACK_LIBRARY_PATH='{dyld}'\n\
                 export WINEPREFIX='{prefix}'\n\
                 export WINEDEBUG='-all'\n\
                 nohup '{wine}' '{terminal}' '/config:{config}' \
                     >/dev/null 2>&1 &\n",
                dyld     = dyld,
                prefix   = wine_prefix.display(),
                wine     = wine_exe,
                terminal = terminal_host.display(),
                config   = config_win,
            );

            let script_path = std::env::temp_dir().join("mt5_backtest_launch.sh");
            fs::write(&script_path, &script)?;
            // chmod +x
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&script_path,
                    fs::Permissions::from_mode(0o755))?;
            }

            tracing::info!("Launching MT5 via shell script: {}", script_path.display());
            let mut cmd = Command::new("/bin/sh");
            cmd.arg(&script_path);
            return Ok(cmd);
        }

        // CrossOver / Linux: invoke wine64 directly with WINEPREFIX set.
        // Params are already written to terminal.ini so no /config: arg needed.
        let terminal_win_path = r"C:\Program Files\MetaTrader 5\terminal64.exe";
        let mut cmd = Command::new(wine_exe);
        cmd.arg(terminal_win_path)
            .env("WINEPREFIX", wine_prefix)
            .env("WINEDEBUG", "-all");
        Ok(cmd)
    }

    /// For terminal.ini: path relative to MQL5/ (e.g. `Experts\DPS21\DPS21.ex5`).
    fn resolve_expert_path(&self, expert: &str) -> String {
        if let Some(experts_dir) = &self.config.experts_dir {
            let nested_ex5 = PathBuf::from(experts_dir).join(expert).join(format!("{}.ex5", expert));
            let nested_mq5 = PathBuf::from(experts_dir).join(expert).join(format!("{}.mq5", expert));
            if nested_ex5.exists() || nested_mq5.exists() {
                return format!("Experts\\{}\\{}.ex5", expert, expert);
            }
        }
        format!("Experts\\{}.ex5", expert)
    }

    /// For /config: INI: path relative to MQL5/Experts/ (e.g. `DPS21\DPS21.ex5`).
    /// The /config: format does NOT include the "Experts\" prefix.
    fn resolve_backtest_ini_expert_path(&self, expert: &str) -> String {
        if let Some(experts_dir) = &self.config.experts_dir {
            let nested_ex5 = PathBuf::from(experts_dir).join(expert).join(format!("{}.ex5", expert));
            let nested_mq5 = PathBuf::from(experts_dir).join(expert).join(format!("{}.mq5", expert));
            if nested_ex5.exists() || nested_mq5.exists() {
                return format!("{}\\{}.ex5", expert, expert);
            }
        }
        format!("{}.ex5", expert)
    }

    fn build_backtest_ini(&self, params: &BacktestParams, report_id: &str) -> Result<String> {
        let mut ini = String::new();

        if let Some(login) = &self.config.backtest_login {
            if let Some(server) = &self.config.backtest_server {
                ini.push_str("[Common]\n");
                ini.push_str(&format!("Login={}\n", login));
                ini.push_str(&format!("Server={}\n\n", server));
            }
        }

        ini.push_str("[Tester]\n");
        // Expert path is relative to MQL5/Experts/ in the /config: format (no "Experts\" prefix).
        ini.push_str(&format!("Expert={}\n", self.resolve_backtest_ini_expert_path(&params.expert)));
        ini.push_str(&format!("Symbol={}\n", params.symbol));
        ini.push_str(&format!("Period={}\n", params.timeframe));
        ini.push_str("Optimization=0\n");
        ini.push_str(&format!("Model={}\n", params.model));
        ini.push_str(&format!("FromDate={}\n", params.from_date));
        ini.push_str(&format!("ToDate={}\n", params.to_date));
        ini.push_str("ForwardMode=0\n");
        ini.push_str(&format!("Deposit={}\n", params.deposit));
        ini.push_str(&format!("Currency={}\n", self.config.backtest_currency.as_ref().unwrap_or(&"USD".to_string())));
        ini.push_str("ProfitInPips=1\n");
        ini.push_str(&format!("Leverage={}\n", params.leverage));
        ini.push_str("Execution=10\n");
        ini.push_str(&format!("Visual={}\n", if params.gui { "1" } else { "0" }));
        ini.push_str(&format!("Report=reports\\{}.htm\n", report_id));
        ini.push_str("ReplaceReport=1\n");
        ini.push_str(&format!("ShutdownTerminal={}\n", if params.shutdown { "1" } else { "0" }));

        if let Some(set_file) = &params.set_file {
            ini.push_str(&format!("ExpertParameters={}\n", set_file));
        }

        Ok(ini)
    }

    async fn kill_mt5(&self) -> Result<()> {
        let patterns = Self::mt5_process_patterns();

        let running = patterns.iter().any(|pat| {
            Command::new("pgrep")
                .args(["-f", pat.as_str()])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });

        if !running {
            return Ok(());
        }

        tracing::info!("Stopping existing MT5 instance...");
        // SIGKILL immediately — MT5 holds no state we care about preserving.
        for pat in &patterns {
            let _ = Command::new("pkill").args(["-KILL", "-f", pat.as_str()]).output();
        }
        let _ = Command::new("pkill").args(["-KILL", "-f", "wineserver"]).output();
        sleep(Duration::from_secs(3)).await;

        Ok(())
    }

    fn is_mt5_running() -> bool {
        Self::mt5_process_patterns().iter().any(|pat| {
            Command::new("pgrep")
                .args(["-f", pat.as_str()])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
    }

    /// Scan `dir` for the newest .htm/.htm.xml/.html file written after `since`.
    fn find_newest_report(dir: &Path, since: std::time::SystemTime) -> Option<PathBuf> {
        let entries = fs::read_dir(dir).ok()?;
        let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let ext = e.path().extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                matches!(ext.as_str(), "htm" | "xml" | "html")
            })
            .filter_map(|e| {
                let mtime = e.metadata().ok()?.modified().ok()?;
                if mtime >= since { Some((mtime, e.path())) } else { None }
            })
            .collect();

        candidates.sort_by_key(|(t, _)| *t);
        candidates.into_iter().last().map(|(_, p)| p)
    }

    fn mt5_process_patterns() -> Vec<String> {
        if cfg!(target_os = "macos") {
            // macOS: official MT5.app bundle (contains its own Wine runtime)
            // Also match Wine-hosted terminal64.exe for CrossOver installs
            vec![
                "MetaTrader 5\\.app".to_string(),
                "terminal64\\.exe".to_string(),
            ]
        } else {
            // Linux: MT5 always runs as a Wine process
            vec![
                "terminal64\\.exe".to_string(),
                "metatrader".to_string(),
            ]
        }
    }

    async fn log_progress(&self, log_path: &Path, stage: &str) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let line = format!("{} {}\n", stage, timestamp);
        let _ = fs::write(log_path, line);
    }

    async fn save_metadata(&self, params: &BacktestParams, report_dir: &Path, duration: i64, no_trades: bool) -> Result<()> {
        let metadata = PipelineMetadata {
            expert: params.expert.clone(),
            symbol: params.symbol.clone(),
            timeframe: params.timeframe.clone(),
            from_date: params.from_date.clone(),
            to_date: params.to_date.clone(),
            deposit: params.deposit as f64,
            currency: self.config.backtest_currency.clone().unwrap_or_else(|| "USD".to_string()),
            model: params.model as i32,
            leverage: params.leverage as i32,
            set_file: params.set_file.clone(),
            report_dir: report_dir.to_string_lossy().to_string(),
            duration_seconds: duration,
            files: FilePaths {
                metrics: report_dir.join("metrics.json").to_string_lossy().to_string(),
                analysis: report_dir.join("analysis.json").to_string_lossy().to_string(),
                deals_csv: report_dir.join("deals.csv").to_string_lossy().to_string(),
                deals_json: report_dir.join("deals.json").to_string_lossy().to_string(),
            },
            no_trades,
        };

        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(report_dir.join("pipeline_metadata.json"), json)?;

        Ok(())
    }

    fn generate_report_id(&self, params: &BacktestParams) -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        format!(
            "{}_{}_{}_{}_{}",
            timestamp, params.expert, params.symbol, params.timeframe, params.model
        )
    }
}
