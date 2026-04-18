use anyhow::Result;
use chrono::Datelike;
use serde_json::{json, Value};
use std::path::Path;
use crate::models::Config;

mod system;
mod experts;
mod backtest;
mod optimization;
mod analysis;
mod setfiles;
mod reports;

#[derive(Debug)]
pub struct ToolHandler {
    pub config: Config,
}

impl ToolHandler {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn handle(&self, name: &str, args: &Value) -> Result<Value> {
        match name {
            // System handlers
            "verify_setup" => system::handle_verify_setup(&self.config).await,
            "list_symbols" => system::handle_list_symbols(&self.config).await,
            "healthcheck" => system::handle_healthcheck(&self.config, args).await,
            
            // Expert/Indicator/Script handlers
            "list_experts" => experts::handle_list_experts(&self.config, args).await,
            "list_indicators" => experts::handle_list_indicators(&self.config, args).await,
            "list_scripts" => experts::handle_list_scripts(&self.config, args).await,
            "compile_ea" => experts::handle_compile_ea(&self.config, args).await,
            
            // Backtest handlers
            "run_backtest" => backtest::handle_run_backtest(&self.config, args).await,
            "get_backtest_status" => backtest::handle_get_backtest_status(&self.config, args).await,
            "cache_status" => backtest::handle_cache_status(&self.config).await,
            "clean_cache" => backtest::handle_clean_cache(&self.config, args).await,
            
            // Optimization handlers
            "run_optimization" => optimization::handle_run_optimization(&self.config, args).await,
            "get_optimization_status" => optimization::handle_get_optimization_status(&self.config, args).await,
            "get_optimization_results" => optimization::handle_get_optimization_results(&self.config, args).await,
            "list_jobs" => optimization::handle_list_jobs(&self.config).await,
            
            // Analysis handlers
            "analyze_report" => analysis::handle_analyze_report(&self.config, args).await,
            "compare_baseline" => analysis::handle_compare_baseline(&self.config, args).await,
            
            // Set file handlers
            "read_set_file" => setfiles::handle_read_set_file(args).await,
            "write_set_file" => setfiles::handle_write_set_file(args).await,
            "patch_set_file" => setfiles::handle_patch_set_file(args).await,
            "clone_set_file" => setfiles::handle_clone_set_file(args).await,
            "diff_set_files" => setfiles::handle_diff_set_files(args).await,
            "set_from_optimization" => setfiles::handle_set_from_optimization(args).await,
            "describe_sweep" => setfiles::handle_describe_sweep(args).await,
            "list_set_files" => setfiles::handle_list_set_files(&self.config).await,
            
            // Report handlers
            "list_reports" => reports::handle_list_reports(args).await,
            "search_reports" => reports::handle_search_reports(args).await,
            "prune_reports" => reports::handle_prune_reports(&self.config, args).await,
            "tail_log" => reports::handle_tail_log(&self.config, args).await,
            "archive_report" => reports::handle_archive_report(&self.config, args).await,
            "archive_all_reports" => reports::handle_archive_all_reports(&self.config, args).await,
            "promote_to_baseline" => reports::handle_promote_to_baseline(&self.config, args).await,
            "get_history" => reports::handle_get_history(args).await,
            "annotate_history" => reports::handle_annotate_history(args).await,
            
            _ => Ok(json!({
                "content": [{ "type": "text", "text": format!("Tool '{}' not implemented", name) }],
                "isError": true
            })),
        }
    }
}

// Helper functions used across modules
pub(crate) fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub(crate) fn past_complete_month() -> (String, String) {
    let now = chrono::Utc::now();
    let today = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let last_of_prev = today.pred_opt()
        .unwrap_or(today);
    let first_of_prev = chrono::NaiveDate::from_ymd_opt(last_of_prev.year(), last_of_prev.month(), 1)
        .unwrap_or(last_of_prev);
    (
        first_of_prev.format("%Y.%m.%d").to_string(),
        last_of_prev.format("%Y.%m.%d").to_string(),
    )
}
