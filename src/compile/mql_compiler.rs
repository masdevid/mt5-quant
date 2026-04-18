use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::Config;

pub struct MqlCompiler {
    config: Config,
}

pub struct CompileResult {
    pub success: bool,
    pub ex5_path: Option<PathBuf>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub binary_size: u64,
}

impl MqlCompiler {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn compile(&self, source_path: &str) -> Result<CompileResult> {
        let source_path = Path::new(source_path);
        
        if !source_path.exists() {
            return Err(anyhow!("Source file not found: {}", source_path.display()));
        }

        let expert_name = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid source file name"))?;

        let mt5_dir = self.config.mt5_dir()
            .ok_or_else(|| anyhow!("terminal_dir not configured"))?;

        let metaeditor = mt5_dir.join("metaeditor64.exe");
        if !metaeditor.exists() {
            return Err(anyhow!("metaeditor64.exe not found at: {}", metaeditor.display()));
        }

        let mt5_src_dir = mt5_dir.join("MQL5").join("Experts");
        fs::create_dir_all(&mt5_src_dir)?;

        let dest_path = mt5_src_dir.join(format!("{}.mq5", expert_name));
        fs::copy(source_path, &dest_path)?;

        self.sync_include_files(source_path, &mt5_dir)?;

        let wine_prefix = self.get_wine_prefix(&mt5_dir)?;
        let wine_exe = self.config.wine_executable.as_ref()
            .ok_or_else(|| anyhow!("wine_executable not configured"))?;

        let log_file = tempfile::NamedTempFile::new()?.path().to_path_buf();
        let wine_src_path = Self::host_to_wine_path(&dest_path)?;
        let wine_log_path = Self::host_to_wine_path(&log_file)?;

        let _output = Command::new(wine_exe)
            .arg(&metaeditor)
            .arg(format!("/compile:{}", wine_src_path))
            .arg(format!("/log:{}", wine_log_path))
            .env("WINEPREFIX", &wine_prefix)
            .env("WINEDEBUG", "-all")
            .output()?;

        let log_content = if log_file.exists() {
            fs::read_to_string(&log_file).unwrap_or_default()
        } else {
            String::new()
        };

        let errors: Vec<String> = log_content
            .lines()
            .filter(|l| l.to_lowercase().contains("error"))
            .map(|s| s.to_string())
            .collect();

        let warnings: Vec<String> = log_content
            .lines()
            .filter(|l| l.to_lowercase().contains("warning"))
            .map(|s| s.to_string())
            .collect();

        let ex5_path = mt5_src_dir.join(format!("{}.ex5", expert_name));
        
        if !ex5_path.exists() {
            return Ok(CompileResult {
                success: false,
                ex5_path: None,
                errors,
                warnings,
                binary_size: 0,
            });
        }

        let binary_size = fs::metadata(&ex5_path)?.len();

        Ok(CompileResult {
            success: errors.is_empty(),
            ex5_path: Some(ex5_path),
            errors,
            warnings,
            binary_size,
        })
    }

    fn sync_include_files(&self, source_path: &Path, mt5_dir: &Path) -> Result<()> {
        let source_dir = source_path.parent()
            .ok_or_else(|| anyhow!("Source path has no parent"))?;

        if let Some(include_dir) = Self::find_include_dir(source_dir) {
            let mt5_include = mt5_dir.join("MQL5").join("Include");
            fs::create_dir_all(&mt5_include)?;

            let mut synced_total = 0;

            for entry in fs::read_dir(&include_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    let dir_name = path.file_name()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| anyhow!("Invalid directory name"))?;
                    
                    let mt5_dest = mt5_include.join(dir_name);
                    if mt5_dest.exists() {
                        fs::remove_dir_all(&mt5_dest)?;
                    }
                    
                    Self::copy_dir_all(&path, &mt5_dest)?;
                    
                    let count = Self::count_mqh_files(&mt5_dest)?;
                    if count > 0 {
                        synced_total += count;
                    }
                }
            }

            for entry in fs::read_dir(&include_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map(|e| e == "mqh").unwrap_or(false) {
                    let dest = mt5_include.join(path.file_name().unwrap());
                    fs::copy(&path, &dest)?;
                    synced_total += 1;
                }
            }

            if synced_total > 0 {
                tracing::info!("Synced {} .mqh file(s)", synced_total);
            }
        }

        Ok(())
    }

    fn find_include_dir(source_dir: &Path) -> Option<PathBuf> {
        let candidates = [
            source_dir.join("include"),
            source_dir.parent().map(|p| p.join("include")).unwrap_or_default(),
            source_dir.parent().and_then(|p| p.parent()).map(|p| p.join("include")).unwrap_or_default(),
        ];

        for candidate in &candidates {
            if candidate.exists() && candidate.is_dir() {
                return Some(candidate.clone());
            }
        }

        None
    }

    fn count_mqh_files(dir: &Path) -> Result<usize> {
        let mut count = 0;
        for entry in walkdir::WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().extension().map(|e| e == "mqh").unwrap_or(false) {
                count += 1;
            }
        }
        Ok(count)
    }

    fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;
        
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let dest = dst.join(entry.file_name());

            if path.is_dir() {
                Self::copy_dir_all(&path, &dest)?;
            } else {
                fs::copy(&path, &dest)?;
            }
        }

        Ok(())
    }

    fn host_to_wine_path(host_path: &Path) -> Result<String> {
        let abs_path = host_path.canonicalize()?;
        let path_str = abs_path.to_string_lossy();
        
        if path_str.starts_with('/') {
            let wine_path = path_str.replace('/', "\\\\");
            Ok(format!("C:\\{}", &wine_path[1..]))
        } else {
            Ok(path_str.to_string())
        }
    }

    fn get_wine_prefix(&self, mt5_dir: &Path) -> Result<PathBuf> {
        mt5_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow!("Could not determine Wine prefix from MT5 directory"))
    }
}
