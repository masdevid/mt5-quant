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
    pub files_synced: usize,
}

pub struct SyncStats {
    pub dest_mq5: PathBuf,
    pub files_copied: usize,
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

        let mt5_dir = self.config.mt5_dir()
            .ok_or_else(|| anyhow!("terminal_dir not configured"))?;
        let metaeditor = mt5_dir.join("metaeditor64.exe");
        if !metaeditor.exists() {
            return Err(anyhow!("metaeditor64.exe not found at: {}", metaeditor.display()));
        }

        let wine_exe = self.config.wine_executable.as_ref()
            .ok_or_else(|| anyhow!("wine_executable not configured"))?;
        let wine_prefix = self.get_wine_prefix(&mt5_dir)?;

        let experts_dir = mt5_dir.join("MQL5").join("Experts");
        fs::create_dir_all(&experts_dir)?;

        // Sync the full project tree to the Experts directory.
        let sync = self.sync_project_to_experts(source_path, &experts_dir)?;
        tracing::info!(
            "Synced {} file(s) to Experts dir: {}",
            sync.files_copied,
            sync.dest_mq5.display()
        );

        let log_path = wine_prefix.join("drive_c").join("_mt5mcp_compile.log");
        let _ = fs::remove_file(&log_path); // clear stale log before compile

        self.run_metaeditor(wine_exe, &wine_prefix, &metaeditor, &sync.dest_mq5, &log_path)?;

        // MetaEditor writes to the /log: path. Fallback: adjacent {ea_name}.log.
        let log_text = if log_path.exists() {
            Self::read_log(&log_path)
        } else {
            let adjacent = sync.dest_mq5.with_extension("log");
            Self::read_log(&adjacent)
        };
        let _ = fs::remove_file(&log_path);
        tracing::info!("Compile log ({} chars):\n{}", log_text.len(), &log_text[..log_text.len().min(500)]);

        // Log format: "path : error: message" / "path : warning: message"
        let errors: Vec<String> = log_text.lines()
            .filter(|l| {
                let low = l.to_lowercase();
                low.contains(": error:") || (low.contains("error") && !low.contains("0 error") && !low.contains("information"))
            })
            .map(|s| s.to_string())
            .collect();

        let warnings: Vec<String> = log_text.lines()
            .filter(|l| {
                let low = l.to_lowercase();
                low.contains(": warning:") || (low.contains("warning") && !low.contains("0 warning") && !low.contains("information"))
            })
            .map(|s| s.to_string())
            .collect();

        let ex5_path = sync.dest_mq5.with_extension("ex5");
        if !ex5_path.exists() {
            // Ensure there's at least one error message if compilation failed
            let final_errors = if errors.is_empty() {
                vec![format!("Compilation failed. Check compile.log for details. Last 500 chars of log:\n{}",
                    &log_text[log_text.len().saturating_sub(500)..])]
            } else {
                errors
            };
            return Ok(CompileResult {
                success: false,
                ex5_path: None,
                errors: final_errors,
                warnings,
                binary_size: 0,
                files_synced: sync.files_copied,
            });
        }

        let binary_size = fs::metadata(&ex5_path)?.len();
        
        // If there are errors but .ex5 was still created, include full log for debugging
        let final_errors = if !errors.is_empty() && errors.len() < 3 {
            // Append raw log excerpt for context if we only caught few errors
            let mut extended = errors;
            extended.push(format!("[Log excerpt] {}", &log_text[..log_text.len().min(300)]));
            extended
        } else {
            errors
        };
        
        Ok(CompileResult {
            success: final_errors.is_empty(),
            ex5_path: Some(ex5_path),
            errors: final_errors,
            warnings,
            binary_size,
            files_synced: sync.files_copied,
        })
    }

    /// Mirror the project source tree into `experts_dir/{ea_name}/`.
    ///
    /// - Uses the directory containing the `.mq5` as the *project root*.
    /// - Copies the `.mq5` and every `.mqh` found anywhere under the project root,
    ///   preserving relative sub-paths so `#include "sub/file.mqh"` resolves correctly.
    /// - Wipes the destination subdirectory first to remove stale includes.
    pub fn sync_project_to_experts(
        &self,
        source_mq5: &Path,
        experts_dir: &Path,
    ) -> Result<SyncStats> {
        let ea_name = source_mq5
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid source file name"))?;

        let project_root = source_mq5
            .parent()
            .ok_or_else(|| anyhow!("Source file has no parent directory"))?;

        // Destination: Experts/{ea_name}/ — wipe first for a clean slate,
        // unless the source is already inside that directory (avoid self-deletion).
        let dest_dir = experts_dir.join(ea_name);
        let source_already_in_dest = source_mq5.starts_with(&dest_dir);

        if source_already_in_dest {
            // Files are already in the right place; nothing to copy.
            let dest_mq5 = dest_dir.join(format!("{}.mq5", ea_name));
            let files_copied = walkdir::WalkDir::new(&dest_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()).map(|x| x == "mq5" || x == "mqh").unwrap_or(false))
                .count();
            tracing::info!("Source already in Experts dir, skipping sync ({} files)", files_copied);
            return Ok(SyncStats { dest_mq5, files_copied });
        }

        if dest_dir.exists() {
            fs::remove_dir_all(&dest_dir)?;
        }
        fs::create_dir_all(&dest_dir)?;

        let mut files_copied = 0;

        // Copy the main .mq5 file.
        let dest_mq5 = dest_dir.join(format!("{}.mq5", ea_name));
        fs::copy(source_mq5, &dest_mq5)?;
        files_copied += 1;

        // Walk the project root and copy every .mqh, preserving relative paths.
        for entry in walkdir::WalkDir::new(project_root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path == source_mq5 {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("mqh") {
                continue;
            }
            let relative = path.strip_prefix(project_root)
                .map_err(|_| anyhow!("Cannot relativise {}", path.display()))?;
            let dest = dest_dir.join(relative);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest)?;
            files_copied += 1;
        }

        tracing::info!(
            "Project sync: {} → {} ({} file(s))",
            project_root.display(),
            dest_dir.display(),
            files_copied
        );

        Ok(SyncStats { dest_mq5, files_copied })
    }

    /// Run MetaEditor to compile `source_mq5`, writing a log to `log_path`.
    /// Uses a shell script on macOS to avoid SIP stripping DYLD_* vars.
    fn run_metaeditor(
        &self,
        wine_exe: &str,
        wine_prefix: &Path,
        metaeditor: &Path,
        source_mq5: &Path,
        log_path: &Path,
    ) -> Result<()> {
        let wine_src    = self.host_to_wine_path(source_mq5, wine_prefix)?;
        let wine_log    = self.host_to_wine_path(log_path, wine_prefix)
            .unwrap_or_else(|_| "C:\\_mt5mcp_compile.log".into());
        let wine_editor = self.host_to_wine_path(metaeditor, wine_prefix)?;

        if wine_exe.contains("MetaTrader 5.app") {
            let wine_bin  = Path::new(wine_exe);
            let wine_root = wine_bin.parent().and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
                .ok_or_else(|| anyhow!("Cannot derive Wine root"))?;
            let dyld = format!(
                "{}:{}:/usr/lib:/usr/local/lib",
                wine_root.join("lib").join("external").display(),
                wine_root.join("lib").display(),
            );
            // Use host path for metaeditor so wine resolves it reliably.
            let editor_host = wine_prefix.join("drive_c")
                .join("Program Files").join("MetaTrader 5").join("MetaEditor64.exe");
            let script = format!(
                "#!/bin/sh\n\
                 export DYLD_FALLBACK_LIBRARY_PATH='{dyld}'\n\
                 export WINEPREFIX='{prefix}'\n\
                 export WINEDEBUG='-all'\n\
                 # Ensure wineserver is running; MetaEditor exits silently if wine session is cold.\n\
                 pgrep -f wineserver > /dev/null 2>&1 || ('{wine}' wineboot 2>/dev/null; sleep 3)\n\
                 '{wine}' '{editor}' '/compile:{src}' '/log:{log}'\n",
                dyld   = dyld,
                prefix = wine_prefix.display(),
                wine   = wine_exe,
                editor = editor_host.display(),
                src    = wine_src,
                log    = wine_log,
            );
            let script_path = std::env::temp_dir().join("mt5_compile.sh");
            fs::write(&script_path, &script)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
            }
            Command::new("/bin/sh").arg(&script_path).output()?;
        } else {
            Command::new(wine_exe)
                .arg(&wine_editor)
                .arg(format!("/compile:{}", wine_src))
                .arg(format!("/log:{}", wine_log))
                .env("WINEPREFIX", wine_prefix)
                .env("WINEDEBUG", "-all")
                .output()?;
        }
        Ok(())
    }

    /// Convert a host path inside the Wine prefix to a Windows path.
    /// e.g. `{prefix}/drive_c/Program Files/MT5/foo.mq5` → `C:\Program Files\MT5\foo.mq5`
    fn host_to_wine_path(&self, host_path: &Path, wine_prefix: &Path) -> Result<String> {
        let abs = host_path.canonicalize()
            .unwrap_or_else(|_| host_path.to_path_buf());
        let drive_c = wine_prefix.join("drive_c");
        let rel = abs.strip_prefix(&drive_c)
            .map_err(|_| anyhow!(
                "Path {} is outside Wine prefix drive_c ({})",
                abs.display(), drive_c.display()
            ))?;
        let win_path = rel.to_string_lossy().replace('/', "\\");
        Ok(format!("C:\\{}", win_path))
    }

    fn read_log(log_path: &Path) -> String {
        let Ok(raw) = fs::read(log_path) else { return String::new() };
        if raw.starts_with(&[0xFF, 0xFE]) {
            raw[2..].chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .filter_map(|c| char::from_u32(c as u32))
                .collect()
        } else {
            String::from_utf8_lossy(&raw).into_owned()
        }
    }

    fn get_wine_prefix(&self, mt5_dir: &Path) -> Result<PathBuf> {
        mt5_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow!("Could not determine Wine prefix from MT5 directory"))
    }
}
