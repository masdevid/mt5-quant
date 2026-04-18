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

        let ea_name = source_path
            .file_stem().and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid source file name"))?;

        // Stage to /tmp to avoid spaces in path (wine /compile: chokes on spaces)
        let stage_dir = std::path::PathBuf::from(format!("/tmp/mt5_compile_{}", ea_name));
        if stage_dir.exists() {
            fs::remove_dir_all(&stage_dir)?;
        }
        fs::create_dir_all(&stage_dir)?;

        // Sync full project tree into staging dir
        let sync = self.sync_project_to_experts(source_path, &stage_dir)?;
        let staged_mq5 = &sync.dest_mq5;
        tracing::info!("Staged {} file(s) to: {}", sync.files_copied, staged_mq5.display());

        self.run_metaeditor(wine_exe, &wine_prefix, &metaeditor, staged_mq5)?;

        // /log flag (no path) writes log adjacent to source: {ea_name}.log
        let log_path = staged_mq5.with_extension("log");
        let log_text = Self::read_log(&log_path);
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

        let ex5_path = staged_mq5.with_extension("ex5");
        if !ex5_path.exists() {
            let final_errors = if errors.is_empty() {
                vec![format!("Compilation failed. Log:\n{}", &log_text[log_text.len().saturating_sub(500)..])]
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

        Ok(CompileResult {
            success: errors.is_empty(),
            ex5_path: Some(ex5_path),
            errors,
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

    /// Run MetaEditor to compile `source_mq5`.
    /// Uses Unix host path for /compile: and bare /log flag (writes log adjacent to source).
    /// Shell script intermediary required on macOS to preserve DYLD_* vars past SIP.
    fn run_metaeditor(
        &self,
        wine_exe: &str,
        wine_prefix: &Path,
        metaeditor: &Path,
        source_mq5: &Path,
    ) -> Result<()> {
        let mt5_dir = metaeditor.parent().unwrap_or(metaeditor);

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
            let editor_host = wine_prefix.join("drive_c")
                .join("Program Files").join("MetaTrader 5").join("MetaEditor64.exe");
            let script = format!(
                "#!/bin/sh\n\
                 export DYLD_FALLBACK_LIBRARY_PATH='{dyld}'\n\
                 export WINEPREFIX='{prefix}'\n\
                 export WINEDEBUG='-all'\n\
                 cd '{mt5_dir}'\n\
                 '{wine}' '{editor}' '/compile:{src}' /log 2>/dev/null\n",
                dyld    = dyld,
                prefix  = wine_prefix.display(),
                wine    = wine_exe,
                editor  = editor_host.display(),
                mt5_dir = mt5_dir.display(),
                src     = source_mq5.display(),
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
                .arg(metaeditor)
                .arg(format!("/compile:{}", source_mq5.display()))
                .arg("/log")
                .env("WINEPREFIX", wine_prefix)
                .env("WINEDEBUG", "-all")
                .current_dir(mt5_dir)
                .output()?;
        }
        Ok(())
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
