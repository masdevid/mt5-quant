use serde_json::{json, Value};

pub fn tool_check_symbol_data_status() -> Value {
    json!({
        "name": "check_symbol_data_status",
        "description": "Validate if a symbol has sufficient historical tick data for a specified date range before running backtest",
        "inputSchema": {
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol to check (e.g., 'XAUUSDc')"
                },
                "from_date": {
                    "type": "string",
                    "description": "Start date in YYYY.MM.DD format"
                },
                "to_date": {
                    "type": "string",
                    "description": "End date in YYYY.MM.DD format"
                }
            },
            "required": ["symbol", "from_date", "to_date"]
        }
    })
}

pub fn tool_get_backtest_history() -> Value {
    json!({
        "name": "get_backtest_history",
        "description": "List all backtests previously run for a specific EA and/or symbol with summary metrics",
        "inputSchema": {
            "type": "object",
            "properties": {
                "expert": {
                    "type": "string",
                    "description": "EA name to filter by"
                },
                "symbol": {
                    "type": "string",
                    "description": "Symbol to filter by"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)",
                    "default": 10
                }
            }
        }
    })
}

pub fn tool_compare_backtests() -> Value {
    json!({
        "name": "compare_backtests",
        "description": "Compare two or more backtest results side-by-side with key metrics analysis",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dirs": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of report directory paths to compare"
                }
            },
            "required": ["report_dirs"]
        }
    })
}

pub fn tool_init_project() -> Value {
    json!({
        "name": "init_project",
        "description": "Create a new MQL5 project with standard directory structure and template files",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Project name (will be used for EA filename)"
                },
                "template": {
                    "type": "string",
                    "enum": ["scalper", "swing", "grid", "basic"],
                    "description": "Project template type",
                    "default": "basic"
                }
            },
            "required": ["name"]
        }
    })
}

pub fn tool_validate_ea_syntax() -> Value {
    json!({
        "name": "validate_ea_syntax",
        "description": "Perform pre-compile syntax check on MQL5 source file without running full compilation",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to .mq5 source file"
                }
            },
            "required": ["path"]
        }
    })
}

pub fn tool_check_mt5_status() -> Value {
    json!({
        "name": "check_mt5_status",
        "description": "Check if MT5 terminal is installed, properly configured, and ready to run",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_create_set_template() -> Value {
    json!({
        "name": "create_set_template",
        "description": "Generate a .set parameter file template based on EA's input variables",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ea": {
                    "type": "string",
                    "description": "EA name or path to .mq5/.ex5 file"
                },
                "output_path": {
                    "type": "string",
                    "description": "Optional output path for .set file"
                }
            },
            "required": ["ea"]
        }
    })
}

pub fn tool_export_report() -> Value {
    json!({
        "name": "export_report",
        "description": "Export backtest report to various formats (CSV, JSON, Markdown)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": {
                    "type": "string",
                    "description": "Path to backtest report directory"
                },
                "format": {
                    "type": "string",
                    "enum": ["csv", "json", "md"],
                    "description": "Export format",
                    "default": "csv"
                },
                "output_path": {
                    "type": "string",
                    "description": "Optional custom output file path"
                }
            },
            "required": ["report_dir"]
        }
    })
}
