use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationPass {
    pub pass: u32,
    pub profit: f64,
    pub total_trades: u32,
    pub profit_factor: f64,
    pub expected_payoff: f64,
    pub drawdown_pct: f64,
    pub params: HashMap<String, String>,
}

pub struct OptimizationParser;

impl OptimizationParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_job(&self, job_id: &str) -> Result<Vec<OptimizationPass>> {
        let jobs_dir = Path::new(".mt5mcp_jobs");
        let meta_path = jobs_dir.join(format!("{}.json", job_id));

        if !meta_path.exists() {
            return Err(anyhow!("Job not found: {}. Check .mt5mcp_jobs/", job_id));
        }

        let meta: serde_json::Value = serde_json::from_str(&fs::read_to_string(&meta_path)?)?;
        let wine_prefix = meta.get("wine_prefix")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("wine_prefix not in job metadata"))?;

        let base_path = Path::new(wine_prefix).join("drive_c/mt5mcp_opt_report");
        
        // Try different extensions
        for ext in &[".htm", ".htm.xml", ".html"] {
            let candidate = base_path.with_extension(ext.trim_start_matches('.'));
            if candidate.exists() {
                return self.parse_file(&candidate);
            }
        }

        Err(anyhow!(
            "Optimization report not found. Expected: {}.htm or {}.htm.xml\nIs MT5 optimization still running?",
            base_path.display(),
            base_path.display()
        ))
    }

    pub fn parse_file(&self, path: &Path) -> Result<Vec<OptimizationPass>> {
        let format = self.detect_format(path);
        let text = self.read_text(path)?;

        match format {
            "xml" => self.parse_xml(&text),
            _ => self.parse_html(&text),
        }
    }

    fn detect_format(&self, path: &Path) -> &str {
        let path_str = path.to_string_lossy();
        if path_str.ends_with(".xml") || path_str.ends_with(".htm.xml") {
            return "xml";
        }
        
        if let Ok(header) = fs::read(path) {
            let header = &header[..header.len().min(512)];
            if header.windows(5).any(|w| w == b"<?xml") || header.windows(8).any(|w| w == b"Workbook") {
                return "xml";
            }
        }
        
        "html"
    }

    fn read_text(&self, path: &Path) -> Result<String> {
        let raw = fs::read(path)?;
        
        // Try UTF-16 first (common for MT5 reports)
        if raw.len() >= 2 {
            if raw[0] == 0xFF && raw[1] == 0xFE {
                // UTF-16 LE with BOM
                let u16_vec: Vec<u16> = raw[2..].chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                return Ok(String::from_utf16_lossy(&u16_vec));
            } else if raw[0] == 0xFE && raw[1] == 0xFF {
                // UTF-16 BE with BOM
                let u16_vec: Vec<u16> = raw[2..].chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                return Ok(String::from_utf16_lossy(&u16_vec));
            }
        }
        
        // Try UTF-8, then fallback to lossy
        if let Ok(text) = String::from_utf8(raw.clone()) {
            return Ok(text);
        }
        
        // Try UTF-16 without BOM
        if raw.len() % 2 == 0 {
            let u16_vec: Vec<u16> = raw.chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let text = String::from_utf16_lossy(&u16_vec);
            if text.chars().any(|c| c.is_ascii_alphanumeric()) {
                return Ok(text);
            }
        }
        
        Ok(String::from_utf8_lossy(&raw).to_string())
    }

    fn parse_html(&self, text: &str) -> Result<Vec<OptimizationPass>> {
        let mut results = Vec::new();
        let mut headers: Vec<String> = Vec::new();
        
        // Find all table rows
        let row_regex = regex::Regex::new(r"<tr[^>]*>(.*?)</tr>")?;
        let cell_regex = regex::Regex::new(r"<t[dh][^>]*>(.*?)</t[dh]>")?;
        let tag_regex = regex::Regex::new(r"<[^>]+>")?;
        
        for row_caps in row_regex.captures_iter(text) {
            let row = &row_caps[1];
            let cells: Vec<String> = cell_regex.captures_iter(row)
                .map(|c| {
                    let cell = &c[1];
                    tag_regex.replace_all(cell, "").trim().to_string().replace(',', "")
                })
                .collect();
            
            if cells.is_empty() {
                continue;
            }
            
            // Header row detection
            if headers.is_empty() && cells[0].to_lowercase().contains("pass") {
                headers = cells;
                continue;
            }
            
            // Data row
            if !headers.is_empty() && cells[0].parse::<u32>().is_ok() {
                let row_map: HashMap<String, String> = headers.iter()
                    .zip(cells.iter())
                    .map(|(h, c)| (h.to_lowercase().replace(' ', "_"), c.clone()))
                    .collect();
                
                if let Some(pass) = self.row_to_pass(&row_map) {
                    results.push(pass);
                }
            }
        }
        
        Ok(results)
    }

    fn parse_xml(&self, text: &str) -> Result<Vec<OptimizationPass>> {
        let mut results = Vec::new();
        
        // Parse SpreadsheetML XML
        let doc = roxmltree::Document::parse(text)?;
        
        // Find all rows in Worksheet/Table
        for node in doc.descendants() {
            if node.has_tag_name(("http://schemas.microsoft.com/office/excel/2003/xml", "Row")) || 
               node.has_tag_name("Row") {
                let cells: Vec<String> = node.children()
                    .filter(|n: &roxmltree::Node<'_, '_>| {
                        n.has_tag_name(("http://schemas.microsoft.com/office/excel/2003/xml", "Cell")) ||
                        n.has_tag_name("Cell") ||
                        n.has_tag_name(("http://schemas.microsoft.com/office/excel/2003/xml", "Data")) ||
                        n.has_tag_name("Data")
                    })
                    .map(|n| n.text().unwrap_or("").trim().to_string().replace(',', ""))
                    .collect();
                
                if cells.is_empty() {
                    continue;
                }
                
                // Check if first cell is a pass number
                if let Ok(pass_num) = cells[0].parse::<u32>() {
                    if pass_num > 0 {
                        let mut row_map = HashMap::new();
                        
                        // Standard MT5 optimization report columns
                        let headers = vec![
                            "pass", "result", "profit", "total_trades", "profit_factor",
                            "expected_payoff", "drawdown_pct", "recovery_factor", "sharpe_ratio",
                            "custom", "consecutive_wins", "consecutive_losses",
                        ];
                        
                        for (i, cell) in cells.iter().enumerate() {
                            if let Some(header) = headers.get(i) {
                                row_map.insert(header.to_string(), cell.clone());
                            }
                        }
                        
                        if let Some(pass) = self.row_to_pass(&row_map) {
                            results.push(pass);
                        }
                    }
                }
            }
        }
        
        Ok(results)
    }

    fn row_to_pass(&self, row: &HashMap<String, String>) -> Option<OptimizationPass> {
        let pass = row.get("pass").or_else(|| row.get("#"))
            .and_then(|v| v.parse().ok())?;
        
        let profit = row.get("profit").or_else(|| row.get("total_net_profit"))
            .and_then(|v| v.replace(' ', "").parse().ok())?;
        
        let total_trades = row.get("total_trades").or_else(|| row.get("trades"))
            .and_then(|v| v.parse().ok())?;
        
        let profit_factor = row.get("profit_factor")
            .and_then(|v| v.parse().ok())?;
        
        let expected_payoff = row.get("expected_payoff")
            .and_then(|v| v.parse().ok())?;
        
        let drawdown_pct = row.get("drawdown_pct").or_else(|| row.get("max_drawdown"))
            .and_then(|v| v.trim_end_matches('%').trim().parse().ok())?;
        
        // Extract parameter values from row
        let params: HashMap<String, String> = row.iter()
            .filter(|(k, _)| ![
                "pass", "result", "profit", "total_trades", "profit_factor",
                "expected_payoff", "drawdown_pct", "max_drawdown", "recovery_factor",
                "sharpe_ratio", "custom", "consecutive_wins", "consecutive_losses"
            ].contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        Some(OptimizationPass {
            pass,
            profit,
            total_trades,
            profit_factor,
            expected_payoff,
            drawdown_pct,
            params,
        })
    }

    pub fn find_best_pass<'a>(&self, passes: &'a [OptimizationPass], criteria: &str) -> Option<&'a OptimizationPass> {
        match criteria {
            "profit" => passes.iter().max_by(|a, b| a.profit.partial_cmp(&b.profit).unwrap()),
            "profit_factor" => passes.iter().max_by(|a, b| a.profit_factor.partial_cmp(&b.profit_factor).unwrap()),
            "sharpe" => passes.iter().max_by(|a, b| {
                let a_sharpe = a.params.get("sharpe_ratio").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                let b_sharpe = b.params.get("sharpe_ratio").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                a_sharpe.partial_cmp(&b_sharpe).unwrap()
            }),
            "drawdown" => passes.iter().min_by(|a, b| a.drawdown_pct.partial_cmp(&b.drawdown_pct).unwrap()),
            _ => passes.iter().max_by(|a, b| a.profit.partial_cmp(&b.profit).unwrap()),
        }
    }
}
