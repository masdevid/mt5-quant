use anyhow::{anyhow, Result};
use chrono;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::time::{sleep, Duration};

use crate::analytics::{DealAnalyzer, ReportExtractor};
use crate::compile::MqlCompiler;
use crate::models::config::Config;
use crate::models::report::{PipelineMetadata, FilePaths};

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
            self.compile_ea(&params.expert).await?;
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
            &report_dir.to_string_lossy()
        )?;

        if !params.skip_analyze {
            self.log_progress(&progress_log, "ANALYZE").await;
            let analysis = self.analyzer.analyze(&extraction.deals, &extraction.metrics);
            
            let analysis_path = report_dir.join("analysis.json");
            let analysis_json = serde_json::to_string_pretty(&analysis)?;
            fs::write(&analysis_path, analysis_json)?;
        }

        self.log_progress(&progress_log, "DONE").await;
        
        let duration = (chrono::Utc::now() - start_time).num_seconds();
        self.save_metadata(&params, &report_dir, duration).await?;

        Ok(PipelineResult {
            success: true,
            report_dir,
            duration_seconds: duration,
            message: "Backtest completed successfully".to_string(),
        })
    }

    async fn compile_ea(&self, expert: &str) -> Result<()> {
        let search_paths = [
            PathBuf::from(&self.config.get("project_dir")).join("src/experts").join(format!("{}.mq5", expert)),
            PathBuf::from(&self.config.get("project_dir")).join("src").join(format!("{}.mq5", expert)),
            PathBuf::from(&self.config.get("project_dir")).join(format!("{}.mq5", expert)),
            PathBuf::from("src/experts").join(format!("{}.mq5", expert)),
            PathBuf::from("src").join(format!("{}.mq5", expert)),
            PathBuf::from(format!("{}.mq5", expert)),
        ];

        let source_path = search_paths
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| anyhow!("Cannot find {}.mq5", expert))?;

        let result = self.compiler.compile(&source_path.to_string_lossy())?;
        
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

        let wine_prefix = mt5_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow!("Could not determine Wine prefix"))?;

        let reports_dir = mt5_dir.join("reports");
        fs::create_dir_all(&reports_dir)?;

        let ini_path = mt5_dir.join("backtest_config.ini");
        let ini_content = self.build_backtest_ini(params, report_id)?;
        
        let ini_utf16: Vec<u8> = std::iter::once(0xFFu8)
            .chain(std::iter::once(0xFEu8))
            .chain(ini_content.encode_utf16().flat_map(|c| c.to_le_bytes()))
            .collect();
        
        fs::write(&ini_path, ini_utf16)?;

        if params.kill_existing {
            self.kill_mt5().await?;
        }

        let bat_content = if params.shutdown {
            format!(r#"@echo off
cd /d "C:\Program Files\MetaTrader 5"
start /wait terminal64.exe /config:"C:\Program Files\MetaTrader 5\backtest_config.ini"
"#)
        } else {
            format!(r#"@echo off
cd /d "C:\Program Files\MetaTrader 5"
start terminal64.exe /config:"C:\Program Files\MetaTrader 5\backtest_config.ini"
"#)
        };

        let bat_path = wine_prefix.join("drive_c").join("_mt5mcp_run.bat");
        fs::write(&bat_path, bat_content)?;

        let _cmd = format!("cmd.exe /c 'C:\\_mt5mcp_run.bat'");

        if params.shutdown {
            let output = Command::new("timeout")
                .arg(&params.timeout.to_string())
                .arg(wine_exe)
                .arg("cmd.exe")
                .arg("/c")
                .arg("C:\\_mt5mcp_run.bat")
                .env("WINEPREFIX", &wine_prefix)
                .env("WINEDEBUG", "-all")
                .output()?;

            if !output.status.success() {
                tracing::warn!("MT5 exited with code: {:?}", output.status.code());
            }
        } else {
            Command::new("nohup")
                .arg(wine_exe)
                .arg("cmd.exe")
                .arg("/c")
                .arg("C:\\_mt5mcp_run.bat")
                .env("WINEPREFIX", &wine_prefix)
                .env("WINEDEBUG", "-all")
                .spawn()?;

            sleep(Duration::from_secs(5)).await;

            let deadline = tokio::time::Instant::now() + Duration::from_secs(params.timeout);
            loop {
                if tokio::time::Instant::now() > deadline {
                    return Err(anyhow!("Timeout waiting for backtest report"));
                }

                for ext in &[".htm", ".htm.xml", ".html"] {
                    let candidate = reports_dir.join(format!("{}{}", report_id, ext));
                    if candidate.exists() {
                        let _ = fs::remove_file(&bat_path);
                        return Ok(candidate);
                    }
                }

                sleep(Duration::from_secs(5)).await;
            }
        }

        for ext in &[".htm", ".htm.xml", ".html"] {
            let candidate = reports_dir.join(format!("{}{}", report_id, ext));
            if candidate.exists() {
                let _ = fs::remove_file(&bat_path);
                return Ok(candidate);
            }
        }

        Err(anyhow!("No report file generated"))
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
        ini.push_str(&format!("Expert={}.ex5\n", params.expert));
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
        ini.push_str("ExecutionMode=10\n");
        ini.push_str("OptimizationCriterion=0\n");
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
        let _output = Command::new("pkill")
            .args(&["-TERM", "-f", "terminal64\\.exe"])
            .output()?;

        sleep(Duration::from_secs(5)).await;

        let check = Command::new("pgrep")
            .args(&["-f", "terminal64\\.exe"])
            .output()?;

        if check.status.success() {
            let _ = Command::new("pkill")
                .args(&["-KILL", "-f", "terminal64\\.exe"])
                .output();
            sleep(Duration::from_secs(1)).await;
        }

        Ok(())
    }

    async fn log_progress(&self, log_path: &Path, stage: &str) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let line = format!("{} {}\n", stage, timestamp);
        let _ = fs::write(log_path, line);
    }

    async fn save_metadata(&self, params: &BacktestParams, report_dir: &Path, duration: i64) -> Result<()> {
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
