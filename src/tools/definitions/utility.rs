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

pub fn tool_diagnose_wine() -> Value {
    json!({
        "name": "diagnose_wine",
        "description": "Check Wine installation, version, and prefix health. Reports errors, warnings, and recent Wine errors.",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_get_mt5_logs() -> Value {
    json!({
        "name": "get_mt5_logs",
        "description": "Get MT5 terminal, tester, or MetaEditor logs with optional search filtering",
        "inputSchema": {
            "type": "object",
            "properties": {
                "log_type": {
                    "type": "string",
                    "enum": ["terminal", "tester", "metaeditor"],
                    "default": "terminal",
                    "description": "Type of log to retrieve"
                },
                "lines": {
                    "type": "integer",
                    "default": 100,
                    "description": "Number of lines to return (from end of log)"
                },
                "search": {
                    "type": "string",
                    "description": "Optional search term to filter log lines"
                }
            }
        }
    })
}

pub fn tool_search_mt5_errors() -> Value {
    json!({
        "name": "search_mt5_errors",
        "description": "Search MT5 logs for error patterns (error, failed, crash, exception, etc.) in recent hours",
        "inputSchema": {
            "type": "object",
            "properties": {
                "hours_back": {
                    "type": "integer",
                    "default": 24,
                    "description": "Hours to search back in logs"
                },
                "max_errors": {
                    "type": "integer",
                    "default": 50,
                    "description": "Maximum number of errors to return"
                }
            }
        }
    })
}

pub fn tool_check_mt5_process() -> Value {
    json!({
        "name": "check_mt5_process",
        "description": "Check if MT5 processes are running, get process info (PID, CPU, memory usage)",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_kill_mt5_process() -> Value {
    json!({
        "name": "kill_mt5_process",
        "description": "Kill stuck MT5 processes. Use force=true for stuck wineserver.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pid": {
                    "type": "string",
                    "description": "Optional specific PID to kill"
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "Use SIGKILL (-9) instead of SIGTERM (-15), also kills wineserver"
                }
            }
        }
    })
}

pub fn tool_check_system_resources() -> Value {
    json!({
        "name": "check_system_resources",
        "description": "Check disk space, memory, and CPU. Warns if resources are low for MT5 operations.",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_validate_mt5_config() -> Value {
    json!({
        "name": "validate_mt5_config",
        "description": "Validate MT5 configuration files (terminal.ini, tester settings). Reports errors and warnings.",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_get_wine_prefix_info() -> Value {
    json!({
        "name": "get_wine_prefix_info",
        "description": "Get detailed Wine prefix information: Windows version, installed programs, registry files, drive_c size",
        "inputSchema": {
            "type": "object"
        }
    })
}

pub fn tool_get_backtest_crash_info() -> Value {
    json!({
        "name": "get_backtest_crash_info",
        "description": "Investigate backtest crashes/failures. Checks for incomplete markers, missing metrics.json, DB deal count, error logs. Can scan recent reports.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "report_dir": {
                    "type": "string",
                    "description": "Optional specific report directory to check"
                },
                "check_recent": {
                    "type": "boolean",
                    "default": true,
                    "description": "Also check recent reports for failures"
                },
                "hours_back": {
                    "type": "integer",
                    "default": 6,
                    "description": "Hours back to check for recent failures"
                }
            }
        }
    })
}
