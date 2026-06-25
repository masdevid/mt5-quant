use anyhow::{anyhow, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::models::Config;

/// Read a file that may be UTF-16LE (with BOM) or UTF-8, returning a UTF-8 String.
/// MT5 .set and .ini files are typically UTF-16LE with BOM (0xFF 0xFE).
fn read_file_as_utf8(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    
    // Check for UTF-16LE BOM (0xFF 0xFE)
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE with BOM - skip the 2-byte BOM and decode
        let utf16_data: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16(&utf16_data)
            .map_err(|e| anyhow!("Failed to decode UTF-16LE: {}", e))
    } else {
        // Try UTF-8
        String::from_utf8(bytes)
            .map_err(|e| anyhow!("Failed to decode as UTF-8: {}", e))
    }
}

pub struct OptimizationParams {
    pub expert: String,
    pub set_file: String,
    pub symbol: String,
    pub from_date: String,
    pub to_date: String,
    pub deposit: u32,
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
        let combinations = self.count_combinations(&params.set_file)
            .map_err(|e| anyhow!("count_combinations failed: {}", e))?;

        // Get paths
        let mt5_dir = self.config.terminal_dir.as_ref()
            .ok_or_else(|| anyhow!("terminal_dir not configured"))?;
        let wine_exe = self.config.wine_executable.as_ref()
            .ok_or_else(|| anyhow!("wine_executable not configured"))?;
        
        // Write .set file as UTF-16LE with BOM directly to MT5 tester directory
        let wine_prefix_dir = self.get_wine_prefix_dir(mt5_dir)?;
        let tester_dir = wine_prefix_dir.join("drive_c/Program Files/MetaTrader 5/MQL5/Profiles/Tester");
        fs::create_dir_all(&tester_dir).map_err(|e| anyhow!("create_dir_all({}) failed: {}", tester_dir.display(), e))?;
        let dst_set_file = tester_dir.join(format!("{}.set", params.expert));
        self.write_utf16le_set(&params.set_file, &dst_set_file)
            .map_err(|e| anyhow!("write_utf16le_set({}) failed: {}", dst_set_file.display(), e))?;

        // Reset OptMode in terminal.ini
        // Patch terminal.ini [Tester] section with optimization params (primary mechanism)
        let terminal_ini = if Path::new(mt5_dir).join("config").exists() {
            Path::new(mt5_dir).join("config").join("terminal.ini")
        } else {
            Path::new(mt5_dir).join("terminal.ini")
        };
        let mt5_ini_text = if terminal_ini.exists() {
            read_file_as_utf8(&terminal_ini).unwrap_or_default()
        } else {
            String::new()
        };
        let expert_path = if let Some(experts_dir) = &self.config.experts_dir {
            let nested = Path::new(experts_dir).join(&params.expert).join(format!("{}.mq5", params.expert));
            if nested.exists() {
                format!("Experts\\{}\\{}.ex5", params.expert, params.expert)
            } else {
                format!("Experts\\{}.ex5", params.expert)
            }
        } else {
            format!("Experts\\{}.ex5", params.expert)
        };
        let tester_section = format!(
            "[Tester]\n\
             Expert={}\n\
             ExpertParameters={}.set\n\
             Symbol={}\n\
             Period=M1\n\
             Model=4\n\
             FromDate={}\n\
             ToDate={}\n\
             ForwardMode=0\n\
             Deposit={}\n\
             Currency={}\n\
             ProfitInPips=0\n\
             Leverage={}\n\
             Execution=10\n\
              Optimization=2\n\
              Visual=0\n\
             Report=reports\\opt_report.htm\n\
             ReplaceReport=1\n\
             ShutdownTerminal=1",
             expert_path, params.expert, params.symbol,
            params.from_date, params.to_date, params.deposit, params.currency, params.leverage,
        );
        let updated_ini = Self::patch_ini_section(&mt5_ini_text, "Tester", &tester_section);
        // Strip any stale [Agents] sections from previous runs (no local agent processes)
        let cleaned = Self::strip_ini_section(&updated_ini, "Agents");
        let final_ini = cleaned.trim_end().to_string();
        let mut utf16_out: Vec<u8> = vec![0xFF, 0xFE];
        utf16_out.extend(final_ini.encode_utf16().flat_map(|c| c.to_le_bytes()));
        fs::write(&terminal_ini, utf16_out)?;

        // Write /config: INI to trigger tester/optimizer mode
        // For /config: format, Expert path is relative to MQL5/Experts/ (no Experts\ prefix)
        let opt_config_win = r"C:\mt5opt_config.ini";
        let opt_config_host = wine_prefix_dir.join("drive_c").join("mt5opt_config.ini");
        let mut opt_ini = String::new();
        if let Some(login) = &self.config.backtest_login {
            if let Some(server) = &self.config.backtest_server {
                opt_ini.push_str("[Common]\n");
                opt_ini.push_str(&format!("Login={}\n", login));
                opt_ini.push_str(&format!("Server={}\n", server));
                if let Some(password) = &self.config.backtest_password {
                    opt_ini.push_str(&format!("Password={}\n", password));
                }
                opt_ini.push_str("\n");
            }
        }
        opt_ini.push_str("[Tester]\n");
        opt_ini.push_str(&format!("Expert={}.ex5\n", params.expert));
        opt_ini.push_str(&format!("ExpertParameters={}.set\n", params.expert));
        opt_ini.push_str(&format!("Symbol={}\n", params.symbol));
        opt_ini.push_str("Period=M1\n");
        opt_ini.push_str("Model=4\n");
        opt_ini.push_str("Optimization=2\n");
        opt_ini.push_str(&format!("FromDate={}\n", params.from_date));
        opt_ini.push_str(&format!("ToDate={}\n", params.to_date));
        opt_ini.push_str("ForwardMode=0\n");
        opt_ini.push_str(&format!("Deposit={}\n", params.deposit));
        opt_ini.push_str(&format!("Currency={}\n", params.currency));
        opt_ini.push_str("ProfitInPips=0\n");
        opt_ini.push_str(&format!("Leverage={}\n", params.leverage));
        opt_ini.push_str("Execution=10\n");
        opt_ini.push_str("Visual=0\n");
        opt_ini.push_str("Report=reports\\opt_report.htm\n");
        opt_ini.push_str("ReplaceReport=1\n");
        opt_ini.push_str("ShutdownTerminal=1\n");
        fs::write(&opt_config_host, opt_ini.as_bytes())?;

        // Build launch script (macOS-compatible with /config: to trigger tester mode)
        let wine_bin = Path::new(wine_exe);
        let wine_root = wine_bin
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow!("Cannot derive Wine root from wine_exe"))?;
        let ext_libs  = wine_root.join("lib").join("external");
        let wine_libs = wine_root.join("lib");
        let dyld = format!("{}:{}:/usr/lib:/usr/local/lib",
            ext_libs.display(), wine_libs.display());
        let terminal_host = wine_prefix_dir.join("drive_c")
            .join("Program Files").join("MetaTrader 5").join("terminal64.exe");

        let script = format!(
            "#!/bin/sh\n\
             export DYLD_FALLBACK_LIBRARY_PATH='{dyld}'\n\
             export WINEPREFIX='{prefix}'\n\
             export WINEDEBUG='-all'\n\
             nohup '{wine}' '{terminal}' '/config:{config}' >/dev/null 2>&1 &\n",
            dyld     = dyld,
            prefix   = wine_prefix_dir.display(),
            wine     = wine_exe,
            terminal = terminal_host.display(),
            config   = opt_config_win,
        );

        let script_path = std::env::temp_dir().join("mt5opt_launch.sh");
        fs::write(&script_path, &script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
        }

        let child = Command::new("/bin/sh")
            .arg(&script_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("spawn /bin/sh {} failed: {}", script_path.display(), e))?;

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
        let content = read_file_as_utf8(Path::new(set_file))?;
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
        let content = read_file_as_utf8(Path::new(src))?;
        
        // Create parent directory if needed
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        // Remove existing file if read-only from previous run
        if dst.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dst, fs::Permissions::from_mode(0o644));
            }
            let _ = fs::remove_file(dst);
        }

        // Write UTF-16LE with BOM
        let mut utf16_content: Vec<u16> = vec![0xFEFF]; // BOM
        utf16_content.extend(content.encode_utf16());
        
        let bytes: Vec<u8> = utf16_content.iter()
            .flat_map(|&c| vec![(c & 0xFF) as u8, ((c >> 8) & 0xFF) as u8])
            .collect();
        
        fs::write(dst, bytes)?;

        Ok(())
    }

    fn get_wine_prefix_dir(&self, mt5_dir: &str) -> Result<PathBuf> {
        let path = Path::new(mt5_dir);
        // Go up three levels: .../drive_c/Program Files/MetaTrader 5 -> .../net.metaquotes.wine.metatrader5
        // (same as backtest pipeline)
        let prefix_dir = path
            .parent()
            .and_then(|p| p.parent())
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

    /// Replace a [section] in an INI string — removes old content and inserts new.
    fn patch_ini_section(text: &str, section: &str, new_content: &str) -> String {
        let section_header = format!("[{}]", section);
        let mut result = String::new();
        let mut in_section = false;
        let mut section_found = false;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == section_header {
                in_section = true;
                section_found = true;
                continue;
            }
            if in_section {
                if trimmed.starts_with('[') {
                    in_section = false;
                    result.push_str(new_content);
                    if !new_content.ends_with('\n') {
                        result.push('\n');
                    }
                    result.push_str(line);
                    result.push('\n');
                    continue;
                }
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }

        if !section_found {
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(new_content);
            result.push('\n');
        } else if in_section {
            result.push_str(new_content);
            result.push('\n');
        }

        result
    }

    /// Remove all lines belonging to a [section] from the INI text.
    fn strip_ini_section(text: &str, section: &str) -> String {
        let header = format!("[{}]", section);
        let mut result = String::new();
        let mut skipping = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == header {
                skipping = true;
                continue;
            }
            if skipping && trimmed.starts_with('[') {
                skipping = false;
            }
            if !skipping {
                result.push_str(line);
                result.push('\n');
            }
        }
        result
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
