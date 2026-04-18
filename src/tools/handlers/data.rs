use anyhow::Result;
use chrono::Datelike;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use crate::models::Config;

/// Export OHLC bar data from MT5 history files
pub async fn handle_export_ohlc(config: &Config, args: &Value) -> Result<Value> {
    let symbol = args.get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("symbol is required"))?;
    
    let timeframe = args.get("timeframe")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("timeframe is required"))?;
    
    let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("csv");
    let max_bars = args.get("max_bars").and_then(|v| v.as_u64()).unwrap_or(100000) as usize;
    
    // Get MT5 history directory
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 terminal_dir not configured"))?;
    
    let history_dir = Path::new(&mt5_dir).join("history");
    let cache_dir = Path::new(&mt5_dir).join("Tester").join("cache");
    
    // Try to find symbol data in history or cache
    let symbol_upper = symbol.to_uppercase();
    let mut data_found = false;
    let mut data_source = String::new();
    
    // Check for .hst files (history)
    let hst_file = history_dir.join(format!("{}-{}.hst", symbol_upper, timeframe));
    if hst_file.exists() {
        data_found = true;
        data_source = hst_file.to_string_lossy().to_string();
    }
    
    // Check for cache files (tester cache with tick data)
    if !data_found {
        let entries = match fs::read_dir(&cache_dir) {
            Ok(entries) => entries,
            Err(_) => return Err(anyhow::anyhow!("Failed to read cache directory")),
        };
        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&symbol_upper) && name.contains(timeframe) {
                    data_found = true;
                    data_source = entry.path().to_string_lossy().to_string();
                    break;
                }
            }
        }
    }
    
    // Check bases directory for symbol data
    if !data_found {
        let bases_dir = mt5_dir.join("Bases");
        if bases_dir.exists() {
            let entries = match fs::read_dir(&bases_dir) {
                Ok(entries) => entries,
                Err(_) => return Err(anyhow::anyhow!("Failed to read bases directory")),
            };
            for entry in entries {
                if let Ok(entry) = entry {
                    let name = entry.file_name().to_string_lossy().to_string().to_uppercase();
                    if name.contains(&symbol_upper) {
                        data_found = true;
                        data_source = format!("Bases directory: {}", entry.path().display());
                        break;
                    }
                }
            }
        }
    }
    
    // Parse date range
    let from_date = args.get("from_date").and_then(|v| v.as_str());
    let to_date = args.get("to_date").and_then(|v| v.as_str());
    
    // Determine output path
    let output_path = if let Some(path) = args.get("output_path").and_then(|v| v.as_str()) {
        path.to_string()
    } else {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        format!("/tmp/{}_{}_{}.{}"
            , symbol.to_lowercase().replace(' ', "_"),
            timeframe.to_lowercase(),
            timestamp,
            format
        )
    };
    
    // Since we can't directly read MT5's binary formats without complex parsing,
    // we'll provide the data location and suggest using MT5's native export
    // or wine command to export
    
    let response = json!({
        "success": data_found,
        "message": if data_found {
            "OHLC data source located"
        } else {
            "No local OHLC data found. Use MT5's History Center to download data first."
        },
        "symbol": symbol,
        "timeframe": timeframe,
        "data_source": data_source,
        "history_dir": history_dir.to_string_lossy(),
        "cache_dir": cache_dir.to_string_lossy(),
        "suggested_export_method": "Use MT5's History Center → Export, or run MT5 with wine and use FileOpen/Write MQL5 functions",
        "note": "MT5 uses proprietary .hst (history) and .ticks (tick) binary formats. Direct export requires:",
        "options": [
            "1. MT5 History Center GUI → Export to CSV",
            "2. Custom MQL5 EA/script to export via FileOpen/FileWrite",
            "3. Use Wine to run MT5 CLI tools if available"
        ],
        "output_path": output_path,
        "format": format,
        "max_bars": max_bars,
        "date_range": {
            "from": from_date,
            "to": to_date
        },
        "alternative": "For programmatic export, create an MQL5 script that uses CopyRates() and writes to CSV"
    });
    
    Ok(json!({
        "content": [{ "type": "text", "text": response.to_string() }],
        "isError": !data_found
    }))
}

/// Export tick data from MT5 tick database
pub async fn handle_export_ticks(config: &Config, args: &Value) -> Result<Value> {
    let symbol = args.get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("symbol is required"))?;
    
    let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("csv");
    let max_ticks = args.get("max_ticks").and_then(|v| v.as_u64()).unwrap_or(1000000) as usize;
    let include_volume = args.get("include_volume").and_then(|v| v.as_bool()).unwrap_or(true);
    
    // Get MT5 directories
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 terminal_dir not configured"))?;
    
    let ticks_dir = Path::new(&mt5_dir).join("history").join("ticks");
    let bases_dir = mt5_dir.join("Bases");
    
    let symbol_upper = symbol.to_uppercase();
    let mut tick_files = vec![];
    
    // Look for tick files
    if ticks_dir.exists() {
        let entries = match fs::read_dir(&ticks_dir) {
            Ok(entries) => entries,
            Err(_) => return Err(anyhow::anyhow!("Failed to read ticks directory")),
        };
        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.to_uppercase().contains(&symbol_upper) {
                    tick_files.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }
    
    // Check bases directory for tick data
    if bases_dir.exists() {
        let entries = match fs::read_dir(&bases_dir) {
            Ok(entries) => entries,
            Err(_) => return Err(anyhow::anyhow!("Failed to read bases directory")),
        };
        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string().to_uppercase();
                if name.contains(&symbol_upper) && name.contains("TICK") {
                    tick_files.push(format!("Bases: {}", entry.path().display()));
                }
            }
        }
    }
    
    let data_found = !tick_files.is_empty();
    
    // Parse date range
    let today = chrono::Local::now().format("%Y.%m.%d").to_string();
    let from_date = args.get("from_date").and_then(|v| v.as_str()).unwrap_or(&today);
    let to_date = args.get("to_date").and_then(|v| v.as_str()).unwrap_or(&today);
    
    // Determine output path
    let output_path = if let Some(path) = args.get("output_path").and_then(|v| v.as_str()) {
        path.to_string()
    } else {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        format!("/tmp/{}_ticks_{}.{}"
            , symbol.to_lowercase().replace(' ', "_"),
            timestamp,
            format
        )
    };
    
    let response = json!({
        "success": data_found,
        "message": if data_found {
            "Tick data source located"
        } else {
            "No local tick data found. Ticks are typically stored in MT5's Bases folder or downloaded on-demand."
        },
        "symbol": symbol,
        "tick_files_found": tick_files.len(),
        "tick_files": tick_files,
        "ticks_dir": ticks_dir.to_string_lossy(),
        "bases_dir": bases_dir.to_string_lossy(),
        "suggested_export_method": "MT5 tick data export requires:",
        "options": [
            "1. Use CopyTicks() in MQL5 EA/script to export to CSV",
            "2. Enable tick data recording in MT5 then export from Journal",
            "3. Use MT5's Strategy Tester with 'Every tick' mode to generate tick data"
        ],
        "output_path": output_path,
        "format": format,
        "max_ticks": max_ticks,
        "include_volume": include_volume,
        "date_range": {
            "from": from_date,
            "to": to_date
        },
        "mql5_example": format!("CopyTicks({}, ticks, COPY_TICKS_ALL, {}, {}); // Then write to file", symbol, from_date, max_ticks),
        "note": "MT5 tick data is stored in proprietary .ticks format or Bases directory. Direct read requires MQL5 script."
    });
    
    Ok(json!({
        "content": [{ "type": "text", "text": response.to_string() }],
        "isError": !data_found
    }))
}

/// List available OHLC and tick data in MT5
pub async fn handle_list_available_data(config: &Config, args: &Value) -> Result<Value> {
    let symbol_filter = args.get("symbol").and_then(|v| v.as_str());
    let data_type = args.get("data_type").and_then(|v| v.as_str()).unwrap_or("both");
    
    let mt5_dir = config.mt5_dir()
        .ok_or_else(|| anyhow::anyhow!("MT5 terminal_dir not configured"))?;
    
    let mut ohlc_symbols = vec![];
    let mut tick_symbols = vec![];
    
    // Scan History directory for OHLC data
    if data_type == "ohlc" || data_type == "both" {
        let history_dir = Path::new(&mt5_dir).join("history");
        if let Ok(entries) = fs::read_dir(&history_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Look for .hst files
                    if name.ends_with(".hst") {
                        let parts: Vec<&str> = name.trim_end_matches(".hst").split('-').collect();
                        if parts.len() == 2 {
                            let sym = parts[0].to_string();
                            let tf = parts[1].to_string();
                            if let Some(filter) = symbol_filter {
                                if sym.to_uppercase().contains(&filter.to_uppercase()) {
                                    ohlc_symbols.push(json!({"symbol": sym, "timeframe": tf, "file": name}));
                                }
                            } else {
                                ohlc_symbols.push(json!({"symbol": sym, "timeframe": tf, "file": name}));
                            }
                        }
                    }
                }
            }
        }
        
        // Scan Tester cache
        let cache_dir = Path::new(&mt5_dir).join("Tester").join("cache");
        if let Ok(entries) = fs::read_dir(&cache_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Cache files often named like SYMBOL_TIMEHASH
                    if !name.contains('.') {  // No extension usually
                        if let Some(filter) = symbol_filter {
                            if name.to_uppercase().starts_with(&filter.to_uppercase()) {
                                let metadata = fs::metadata(entry.path()).ok();
                                let size = metadata.map(|m| m.len()).unwrap_or(0);
                                ohlc_symbols.push(json!({
                                    "symbol": name.split('_').next().unwrap_or(&name),
                                    "source": "tester_cache",
                                    "cache_file": name,
                                    "size_bytes": size
                                }));
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Scan for tick data
    if data_type == "ticks" || data_type == "both" {
        let bases_dir = mt5_dir.join("Bases");
        if let Ok(entries) = fs::read_dir(&bases_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.contains("TICK") || name.contains("tick") {
                        if let Some(filter) = symbol_filter {
                            if name.to_uppercase().contains(&filter.to_uppercase()) {
                                tick_symbols.push(json!({"file": name, "type": "tick_database"}));
                            }
                        } else {
                            tick_symbols.push(json!({"file": name, "type": "tick_database"}));
                        }
                    }
                }
            }
        }
    }
    
    let response = json!({
        "success": true,
        "mt5_directory": mt5_dir,
        "data_type_filtered": data_type,
        "symbol_filter": symbol_filter,
        "ohlc_data": {
            "count": ohlc_symbols.len(),
            "sources": [
                format!("{}/history/*.hst", mt5_dir.display()),
                format!("{}/Tester/cache/*", mt5_dir.display())
            ],
            "symbols": ohlc_symbols
        },
        "tick_data": {
            "count": tick_symbols.len(),
            "sources": [
                format!("{}/Bases/*", mt5_dir.display())
            ],
            "files": tick_symbols
        },
        "note": "MT5 uses proprietary binary formats (.hst, .ticks). To export data:",
        "export_methods": [
            "1. Use MT5 History Center → Export (for OHLC)",
            "2. Create MQL5 script using CopyRates()/CopyTicks() → FileWrite CSV",
            "3. Use wine to run MT5 with custom export EA"
        ],
        "mql5_export_script_template": r#"
//+------------------------------------------------------------------+
//| Export OHLC to CSV                                              |
//+------------------------------------------------------------------+
int OnInit() {
    string filename = "Export_" + _Symbol + "_" + EnumToString(Period()) + ".csv";
    int handle = FileOpen(filename, FILE_WRITE|FILE_CSV|FILE_COMMON);
    if(handle != INVALID_HANDLE) {
        FileWrite(handle, "Date,Time,Open,High,Low,Close,Volume");
        MqlRates rates[];
        int copied = CopyRates(_Symbol, PERIOD_CURRENT, 0, 1000, rates);
        for(int i=0; i<copied; i++) {
            FileWrite(handle, 
                TimeToString(rates[i].time, TIME_DATE),
                TimeToString(rates[i].time, TIME_MINUTES),
                rates[i].open, rates[i].high, rates[i].low, rates[i].close, rates[i].tick_volume
            );
        }
        FileClose(handle);
    }
    return(INIT_SUCCEEDED);
}
        "#
    });
    
    Ok(json!({
        "content": [{ "type": "text", "text": response.to_string() }],
        "isError": false
    }))
}
