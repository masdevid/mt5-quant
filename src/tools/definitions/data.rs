use serde_json::{json, Value};

pub fn tool_export_ohlc() -> Value {
    json!({
        "name": "export_ohlc",
        "description": "Export OHLC bar data from MetaTrader history to CSV/JSON",
        "inputSchema": {
            "type": "object",
            "required": ["symbol", "timeframe"],
            "properties": {
                "symbol": { "type": "string", "description": "Trading symbol (e.g., XAUUSDc, EURUSD)" },
                "timeframe": { "type": "string", "enum": ["M1", "M5", "M15", "M30", "H1", "H4", "D1", "W1", "MN1"], "description": "Bar timeframe" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD (default: earliest available)" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD (default: latest available)" },
                "format": { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Output format" },
                "output_path": { "type": "string", "description": "Output file path (default: auto-generated in /tmp)" },
                "max_bars": { "type": "integer", "description": "Maximum bars to export (default: 100000)" }
            }
        }
    })
}

pub fn tool_export_ticks() -> Value {
    json!({
        "name": "export_ticks",
        "description": "Export tick data from MetaTrader tick database to CSV/JSON",
        "inputSchema": {
            "type": "object",
            "required": ["symbol"],
            "properties": {
                "symbol": { "type": "string", "description": "Trading symbol (e.g., XAUUSDc, EURUSD)" },
                "from_date": { "type": "string", "description": "Start date YYYY.MM.DD (default: today)" },
                "to_date": { "type": "string", "description": "End date YYYY.MM.DD (default: today)" },
                "format": { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Output format" },
                "output_path": { "type": "string", "description": "Output file path (default: auto-generated in /tmp)" },
                "max_ticks": { "type": "integer", "description": "Maximum ticks to export (default: 1000000)" },
                "include_volume": { "type": "boolean", "default": true, "description": "Include tick volume" }
            }
        }
    })
}

pub fn tool_list_available_data() -> Value {
    json!({
        "name": "list_available_data",
        "description": "List available OHLC and tick data in MetaTrader history/cache",
        "inputSchema": {
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "Filter by symbol (optional)" },
                "data_type": { "type": "string", "enum": ["ohlc", "ticks", "both"], "default": "both", "description": "Type of data to list" }
            }
        }
    })
}
