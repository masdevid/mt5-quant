use serde_json::Value;

pub mod analytics;
pub mod backtest;
pub mod baseline;
pub mod data;
pub mod experts;
pub mod optimization;
pub mod reports;
pub mod setfiles;
pub mod system;

pub fn get_tools_list() -> Value {
    let tools = vec![
        // Backtest
        backtest::tool_run_backtest(),
        backtest::tool_get_backtest_status(),
        backtest::tool_cache_status(),
        backtest::tool_clean_cache(),
        // Optimization
        optimization::tool_run_optimization(),
        optimization::tool_get_optimization_status(),
        optimization::tool_get_optimization_results(),
        optimization::tool_list_jobs(),
        // Analytics (9 tools)
        analytics::tool_analyze_report(),
        analytics::tool_analyze_monthly_pnl(),
        analytics::tool_analyze_drawdown_events(),
        analytics::tool_analyze_top_losses(),
        analytics::tool_analyze_loss_sequences(),
        analytics::tool_analyze_position_pairs(),
        analytics::tool_analyze_direction_bias(),
        analytics::tool_analyze_streaks(),
        analytics::tool_analyze_concurrent_peak(),
        // Baseline
        baseline::tool_compare_baseline(),
        // Experts
        experts::tool_compile_ea(),
        experts::tool_list_experts(),
        experts::tool_list_indicators(),
        experts::tool_list_scripts(),
        // System
        system::tool_verify_setup(),
        system::tool_list_symbols(),
        system::tool_healthcheck(),
        // Set Files
        setfiles::tool_read_set_file(),
        setfiles::tool_write_set_file(),
        setfiles::tool_patch_set_file(),
        setfiles::tool_clone_set_file(),
        setfiles::tool_diff_set_files(),
        setfiles::tool_describe_sweep(),
        setfiles::tool_list_set_files(),
        setfiles::tool_set_from_optimization(),
        // Data Export (3 tools)
        data::tool_export_ohlc(),
        data::tool_export_ticks(),
        data::tool_list_available_data(),
        // Reports (11 tools)
        reports::tool_list_reports(),
        reports::tool_search_reports(),
        reports::tool_get_latest_report(),
        reports::tool_prune_reports(),
        reports::tool_tail_log(),
        reports::tool_archive_report(),
        reports::tool_archive_all_reports(),
        reports::tool_get_history(),
        reports::tool_promote_to_baseline(),
        reports::tool_annotate_history(),
    ];

    serde_json::json!(tools)
}
