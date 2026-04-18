use anyhow::{anyhow, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::models::Config;

pub struct OptimizationParams {
    pub expert: String,
    pub set_file: String,
    pub symbol: String,
    pub from_date: String,
    pub to_date: String,
    pub deposit: u32,
    pub model: u8,
    pub leverage: u32,
    pub currency: String,
}

impl Default for OptimizationParams {
    fn default() -> Self {
        Self {
            expert: String::new(),
            set_file: String::new(),
            symbol: "XAUUSD".to_string(),
            from_date: String::new(),
            to_date: String::new(),
            deposit: 10000,
            model: 0,
            leverage: 500,
            currency: "USD".to_string(),
        }
    }
}

pub struct OptimizationResult {
    pub success: bool,
    pub job_id: String,
    pub pid: u32,
    pub log_file: PathBuf,
    pub combinations: u64,
    pub message: String,
}

pub struct OptimizationRunner {
    config: Config,
}

impl OptimizationRunner {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self, params: OptimizationParams) -> Result<OptimizationResult> {
        // Validate required fields
        if params.expert.is_empty() {
            return Err(anyhow!("expert is required"));
        }
        if params.set_file.is_empty() {
            return Err(anyhow!("set_file is required"));
        }
        if params.from_date.is_empty() {
            return Err(anyhow!("from_date is required"));
        }
        if params.to_date.is_empty() {
            return Err(anyhow!("to_date is required"));
        }

        let set_path = Path::new(&params.set_file);
        if !set_path.exists() {
            return Err(anyhow!("Set file not found: {}", params.set_file));
        }

        // Generate job ID and log file
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let job_id = format!("opt_{}", timestamp);
        let log_file = PathBuf::from(format!("/tmp/mt5opt_{}.log", timestamp));

        // Count combinations
        let combinations = self.count_combinations(&params.set_file)?;

        // Get paths
        let mt5_dir = self.config.terminal_dir.as_ref()
            .ok_or_else(|| anyhow!("terminal_dir not configured"))?;
        let wine_exe = self.config.wine_executable.as_ref()
            .ok_or_else(|| anyhow!("wine_executable not configured"))?;
        
        // Write .set file as UTF-16LE with BOM directly to MT5 tester directory
        let wine_prefix_dir = self.get_wine_prefix_dir(mt5_dir)?;
        let tester_dir = wine_prefix_dir.join("drive_c/Program Files/MetaTrader 5/MQL5/Profiles/Tester");
        fs::create_dir_all(&tester_dir)?;
        let dst_set_file = tester_dir.join(format!("{}.set", params.expert));
        self.write_utf16le_set(&params.set_file, &dst_set_file)?;

        // Reset OptMode in terminal.ini
        self.reset_optmode(mt5_dir)?;

        // Get Wine prefix directory
        let wine_prefix_dir = self.get_wine_prefix_dir(mt5_dir)?;

        // Build optimization INI
        let ini_path = wine_prefix_dir.join("drive_c/mt5mcp_backtest.ini");
        let ini_content = format!(r#"[Tester]
Expert={}
Symbol={}
Period=M5
Deposit={}
Currency={}
Leverage={}
Model={}
FromDate={}
ToDate={}
Report=C:\mt5mcp_opt_report
Optimization=2
ExpertParameters={}.set
ShutdownTerminal=1
"#, params.expert, params.symbol, params.deposit, params.currency, 
            params.leverage, params.model, params.from_date, params.to_date, params.expert);
        fs::write(&ini_path, ini_content)?;

        // Build batch file
        let batch_path = wine_prefix_dir.join("drive_c/mt5mcp_run.bat");
        let batch_content = format!(r#"@echo off
"C:\Program Files\MetaTrader 5\terminal64.exe" /config:C:\mt5mcp_backtest.ini
"#);
        fs::write(&batch_path, batch_content)?;

        // Launch detached process
        let cmd = format!("cmd.exe /c 'C:\\mt5mcp_run.bat'");
        let child = Command::new(wine_exe)
            .arg(&cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let pid = child.id();

        // Write job metadata
        self.write_job_metadata(&job_id, pid, &params, &log_file, combinations, &wine_prefix_dir)?;

        Ok(OptimizationResult {
            success: true,
            job_id,
            pid,
            log_file,
            combinations,
            message: format!("Optimization launched (pid: {}). Runs for 2-6 hours. Do NOT kill this process.", pid),
        })
    }

    fn count_combinations(&self, set_file: &str) -> Result<u64> {
        let content = fs::read_to_string(set_file)?;
        let mut total: u64 = 1;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(';') || !line.contains('=') {
                continue;
            }

            // Format: param=value||start||step||stop||Y
            let parts: Vec<&str> = line.split("||").collect();
            if parts.len() >= 5 && parts.last().unwrap().trim().to_uppercase() == "Y" {
                if let (Ok(start), Ok(step), Ok(stop)) = (
                    parts[1].trim().parse::<f64>(),
                    parts[2].trim().parse::<f64>(),
                    parts[3].trim().parse::<f64>(),
                ) {
                    if step > 0.0 {
                        let count = ((stop - start) / step).max(0.0) as u64 + 1;
                        total = total.saturating_mul(count);
                    }
                }
            }
        }

        Ok(total.max(1))
    }

    fn write_utf16le_set(&self, src: &str, dst: &Path) -> Result<()> {
        let content = fs::read_to_string(src)?;
        
        // Create parent directory if needed
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write UTF-16LE with BOM
        let mut utf16_content: Vec<u16> = vec![0xFEFF]; // BOM
        utf16_content.extend(content.encode_utf16());
        
        let bytes: Vec<u8> = utf16_content.iter()
            .flat_map(|&c| vec![(c & 0xFF) as u8, ((c >> 8) & 0xFF) as u8])
            .collect();
        
        fs::write(dst, bytes)?;
        
        // Make read-only
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dst, fs::Permissions::from_mode(0o444))?;
        }

        Ok(())
    }

    fn reset_optmode(&self, mt5_dir: &str) -> Result<()> {
        let terminal_ini = Path::new(mt5_dir).join("terminal.ini");
        
        if !terminal_ini.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&terminal_ini)?;
        let updated = content
            .lines()
            .map(|line| {
                if line.starts_with("OptMode=") {
                    "OptMode=0".to_string()
                } else if line.starts_with("LastOptimization=") {
                    String::new()
                } else {
                    line.to_string()
                }
            })
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&terminal_ini, updated)?;
        Ok(())
    }

    fn get_wine_prefix_dir(&self, mt5_dir: &str) -> Result<PathBuf> {
        let path = Path::new(mt5_dir);
        // Go up two levels: .../drive_c/Program Files/MetaTrader 5 -> .../drive_c
        let prefix_dir = path
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow!("Cannot determine Wine prefix from terminal_dir"))?;
        Ok(prefix_dir.to_path_buf())
    }

    fn write_job_metadata(
        &self,
        job_id: &str,
        pid: u32,
        params: &OptimizationParams,
        log_file: &Path,
        combinations: u64,
        wine_prefix: &Path,
    ) -> Result<()> {
        let jobs_dir = Path::new(".mt5mcp_jobs");
        fs::create_dir_all(jobs_dir)?;

        let meta_path = jobs_dir.join(format!("{}.json", job_id));
        let started_at = Utc::now().to_rfc3339();

        let metadata = serde_json::json!({
            "job_id": job_id,
            "pid": pid,
            "expert": params.expert,
            "symbol": params.symbol,
            "from_date": params.from_date,
            "to_date": params.to_date,
            "set_file": params.set_file,
            "combinations": combinations,
            "log_file": log_file.to_string_lossy(),
            "wine_prefix": wine_prefix.to_string_lossy(),
            "started_at": started_at,
        });

        fs::write(&meta_path, serde_json::to_string_pretty(&metadata)?)?;
        Ok(())
    }

    pub fn get_job_status(&self, job_id: &str) -> Result<serde_json::Value> {
        let jobs_dir = Path::new(".mt5mcp_jobs");
        let meta_path = jobs_dir.join(format!("{}.json", job_id));

        if !meta_path.exists() {
            return Ok(serde_json::json!({
                "status": "not_found",
                "message": format!("Job {} not found", job_id)
            }));
        }

        let meta: serde_json::Value = serde_json::from_str(&fs::read_to_string(&meta_path)?)?;
        let pid = meta.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        // Check if process is still running
        let is_running = self.is_process_running(pid);

        // Check for completion marker in log
        let log_file = meta.get("log_file").and_then(|v| v.as_str()).unwrap_or("");
        let is_complete = if !log_file.is_empty() && Path::new(log_file).exists() {
            fs::read_to_string(log_file)
                .map(|content| content.contains("Optimization complete"))
                .unwrap_or(false)
        } else {
            false
        };

        let status = if is_complete {
            "completed"
        } else if is_running {
            "running"
        } else {
            "stopped"
        };

        Ok(serde_json::json!({
            "status": status,
            "job_id": job_id,
            "pid": pid,
            "expert": meta.get("expert"),
            "symbol": meta.get("symbol"),
            "started_at": meta.get("started_at"),
            "log_file": log_file,
        }))
    }

    fn is_process_running(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
        #[cfg(windows)]
        {
            // Windows implementation would use different method
            false
        }
    }

    pub fn list_jobs(&self) -> Result<Vec<serde_json::Value>> {
        let jobs_dir = Path::new(".mt5mcp_jobs");
        let mut jobs = Vec::new();

        if jobs_dir.exists() {
            for entry in fs::read_dir(jobs_dir)? {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().map(|e| e == "json").unwrap_or(false) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                                let job_id = path.file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                
                                let pid = meta.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let is_running = self.is_process_running(pid);
                                
                                jobs.push(serde_json::json!({
                                    "job_id": job_id,
                                    "expert": meta.get("expert"),
                                    "status": if is_running { "running" } else { "stopped" },
                                    "started_at": meta.get("started_at"),
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(jobs)
    }
}
