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
mod utility;

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
            "get_active_account" => system::handle_get_active_account(&self.config).await,
            
            // Expert/Indicator/Script handlers
            "list_experts" => experts::handle_list_experts(&self.config, args).await,
            "list_indicators" => experts::handle_list_indicators(&self.config, args).await,
            "list_scripts" => experts::handle_list_scripts(&self.config, args).await,
            "compile_ea" => experts::handle_compile_ea(&self.config, args).await,
            "search_experts" => experts::handle_search_experts(&self.config, args).await,
            "search_indicators" => experts::handle_search_indicators(&self.config, args).await,
            "search_scripts" => experts::handle_search_scripts(&self.config, args).await,
            "copy_indicator_to_project" => experts::handle_copy_indicator_to_project(&self.config, args).await,
            "copy_script_to_project" => experts::handle_copy_script_to_project(&self.config, args).await,
            
            // Backtest handlers - Granular pipeline options
            "run_backtest" => backtest::handle_run_backtest(&self.config, args).await,           // Full: compile + clean + backtest + extract + analyze
            "run_backtest_quick" => backtest::handle_run_backtest_quick(&self.config, args).await, // Quick: skip compile, do backtest + extract + analyze
            "run_backtest_only" => backtest::handle_run_backtest_only(&self.config, args).await,  // Minimal: skip compile, do backtest + extract only
            "launch_backtest" => backtest::handle_launch_backtest(&self.config, args).await,     // Fire-and-forget mode
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
            "analyze_monthly_pnl" => analysis::handle_analyze_monthly_pnl(&self.config, args).await,
            "analyze_drawdown_events" => analysis::handle_analyze_drawdown_events(&self.config, args).await,
            "analyze_top_losses" => analysis::handle_analyze_top_losses(&self.config, args).await,
            "analyze_loss_sequences" => analysis::handle_analyze_loss_sequences(&self.config, args).await,
            "analyze_position_pairs" => analysis::handle_analyze_position_pairs(&self.config, args).await,
            "analyze_direction_bias" => analysis::handle_analyze_direction_bias(&self.config, args).await,
            "analyze_streaks" => analysis::handle_analyze_streaks(&self.config, args).await,
            "analyze_concurrent_peak" => analysis::handle_analyze_concurrent_peak(&self.config, args).await,
            "compare_baseline" => analysis::handle_compare_baseline(&self.config, args).await,
            // Deal query handlers
            "list_deals" => analysis::handle_list_deals(&self.config, args).await,
            "search_deals_by_comment" => analysis::handle_search_deals_by_comment(&self.config, args).await,
            "search_deals_by_magic" => analysis::handle_search_deals_by_magic(&self.config, args).await,
            "analyze_profit_distribution" => analysis::handle_analyze_profit_distribution(&self.config, args).await,
            "analyze_time_performance" => analysis::handle_analyze_time_performance(&self.config, args).await,
            "analyze_hold_time_distribution" => analysis::handle_analyze_hold_time_distribution(&self.config, args).await,
            "analyze_layer_performance" => analysis::handle_analyze_layer_performance(&self.config, args).await,
            "analyze_volume_vs_profit" => analysis::handle_analyze_volume_vs_profit(&self.config, args).await,
            "analyze_costs" => analysis::handle_analyze_costs(&self.config, args).await,
            "analyze_efficiency" => analysis::handle_analyze_efficiency(&self.config, args).await,
            
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
            "get_latest_report" => reports::handle_get_latest_report(&self.config, args).await,
            "list_reports" => reports::handle_list_reports(args).await,
            "search_reports" => reports::handle_search_reports(args).await,
            "prune_reports" => reports::handle_prune_reports(&self.config, args).await,
            "tail_log" => reports::handle_tail_log(&self.config, args).await,
            "archive_report" => reports::handle_archive_report(&self.config, args).await,
            "archive_all_reports" => reports::handle_archive_all_reports(&self.config, args).await,
            "promote_to_baseline" => reports::handle_promote_to_baseline(&self.config, args).await,
            "get_history" => reports::handle_get_history(args).await,
            "annotate_history" => reports::handle_annotate_history(args).await,
            "get_report_by_id" => reports::handle_get_report_by_id(&self.config, args).await,
            "get_reports_summary" => reports::handle_get_reports_summary(args).await,
            "get_best_reports" => reports::handle_get_best_reports(args).await,
            "search_reports_by_tags" => reports::handle_search_reports_by_tags(args).await,
            "search_reports_by_date_range" => reports::handle_search_reports_by_date_range(args).await,
            "search_reports_by_notes" => reports::handle_search_reports_by_notes(args).await,
            "get_reports_by_set_file" => reports::handle_get_reports_by_set_file(args).await,
            "get_comparable_reports" => reports::handle_get_comparable_reports(args).await,
            
            // Utility handlers
            "check_symbol_data_status" => utility::handle_check_symbol_data_status(&self.config, args).await,
            "get_backtest_history" => utility::handle_get_backtest_history(&self.config, args).await,
            "compare_backtests" => utility::handle_compare_backtests(&self.config, args).await,
            "init_project" => utility::handle_init_project(&self.config, args).await,
            "validate_ea_syntax" => utility::handle_validate_ea_syntax(&self.config, args).await,
            "check_mt5_status" => utility::handle_check_mt5_status(&self.config).await,
            "create_set_template" => utility::handle_create_set_template(&self.config, args).await,
            "export_report" => utility::handle_export_report(&self.config, args).await,
            // Debugging/diagnostics handlers
            "diagnose_wine" => utility::handle_diagnose_wine(&self.config, args).await,
            "get_mt5_logs" => utility::handle_get_mt5_logs(&self.config, args).await,
            "search_mt5_errors" => utility::handle_search_mt5_errors(&self.config, args).await,
            "check_mt5_process" => utility::handle_check_mt5_process(&self.config, args).await,
            "kill_mt5_process" => utility::handle_kill_mt5_process(&self.config, args).await,
            "check_system_resources" => utility::handle_check_system_resources(&self.config, args).await,
            "validate_mt5_config" => utility::handle_validate_mt5_config(&self.config, args).await,
            "get_wine_prefix_info" => utility::handle_get_wine_prefix_info(&self.config, args).await,
            "get_backtest_crash_info" => utility::handle_get_backtest_crash_info(&self.config, args).await,

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
