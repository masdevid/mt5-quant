use serde_json::Value;

pub mod analytics;
pub mod backtest;
pub mod baseline;
pub mod experts;
pub mod optimization;
pub mod reports;
pub mod setfiles;
pub mod system;
pub mod utility;

pub fn get_tools_list() -> Value {
    let tools = vec![
        // Backtest - Granular options
        backtest::tool_run_backtest(),           // Full pipeline: compile + clean + backtest + extract + analyze
        backtest::tool_run_backtest_quick(),     // Quick: skip compile, do clean + backtest + extract + analyze
        backtest::tool_run_backtest_only(),      // Minimal: skip compile, do clean + backtest + extract only
        backtest::tool_launch_backtest(),        // Fire-and-forget: compile + clean + launch MT5
        backtest::tool_get_backtest_status(),    // Poll for completion
        backtest::tool_get_tester_log(),         // Live journal reading mid-backtest or after
        backtest::tool_cache_status(),
        backtest::tool_clean_cache(),
        // Optimization
        optimization::tool_run_optimization(),
        optimization::tool_get_optimization_status(),
        optimization::tool_get_optimization_results(),
        optimization::tool_list_jobs(),
        // Analytics (19 tools)
        analytics::tool_analyze_report(),
        analytics::tool_analyze_monthly_pnl(),
        analytics::tool_analyze_drawdown_events(),
        analytics::tool_analyze_top_losses(),
        analytics::tool_analyze_loss_sequences(),
        analytics::tool_analyze_position_pairs(),
        analytics::tool_analyze_direction_bias(),
        analytics::tool_analyze_streaks(),
        analytics::tool_analyze_concurrent_peak(),
        // Deal query tools (10)
        analytics::tool_list_deals(),
        analytics::tool_search_deals_by_comment(),
        analytics::tool_search_deals_by_magic(),
        analytics::tool_analyze_profit_distribution(),
        analytics::tool_analyze_time_performance(),
        analytics::tool_analyze_hold_time_distribution(),
        analytics::tool_analyze_layer_performance(),
        analytics::tool_analyze_volume_vs_profit(),
        analytics::tool_analyze_costs(),
        analytics::tool_analyze_efficiency(),
        // Baseline
        baseline::tool_compare_baseline(),
        // Experts
        experts::tool_compile_ea(),
        experts::tool_list_experts(),
        experts::tool_list_indicators(),
        experts::tool_list_scripts(),
        experts::tool_search_experts(),
        experts::tool_search_indicators(),
        experts::tool_search_scripts(),
        experts::tool_copy_indicator_to_project(),
        experts::tool_copy_script_to_project(),
        // System
        system::tool_verify_setup(),
        system::tool_list_symbols(),
        system::tool_healthcheck(),
        system::tool_get_active_account(),
        system::tool_check_update(),
        system::tool_update(),
        // Utility (8 tools)
        utility::tool_check_symbol_data_status(),
        utility::tool_get_backtest_history(),
        utility::tool_compare_backtests(),
        utility::tool_init_project(),
        utility::tool_validate_ea_syntax(),
        utility::tool_check_mt5_status(),
        utility::tool_create_set_template(),
        utility::tool_export_report(),
        // Debugging/Diagnostics (10 tools)
        utility::tool_diagnose_wine(),
        utility::tool_get_mt5_logs(),
        utility::tool_search_mt5_errors(),
        utility::tool_check_mt5_process(),
        utility::tool_kill_mt5_process(),
        utility::tool_check_system_resources(),
        utility::tool_validate_mt5_config(),
        utility::tool_get_wine_prefix_info(),
        utility::tool_get_backtest_crash_info(),
        // Set Files
        setfiles::tool_read_set_file(),
        setfiles::tool_write_set_file(),
        setfiles::tool_patch_set_file(),
        setfiles::tool_clone_set_file(),
        setfiles::tool_diff_set_files(),
        setfiles::tool_describe_sweep(),
        setfiles::tool_list_set_files(),
        setfiles::tool_set_from_optimization(),
        // Reports (19 tools)
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
        reports::tool_get_report_by_id(),
        reports::tool_get_reports_summary(),
        reports::tool_get_best_reports(),
        reports::tool_search_reports_by_tags(),
        reports::tool_search_reports_by_date_range(),
        reports::tool_search_reports_by_notes(),
        reports::tool_get_reports_by_set_file(),
        reports::tool_get_comparable_reports(),
        reports::tool_export_deals_csv(),
    ];

    serde_json::json!(tools)
}
