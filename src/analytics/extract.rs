use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::models::{Deal, Metrics};

pub struct ReportExtractor;

impl ReportExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract(&self, report_path: &str, output_dir: &str) -> Result<ExtractionResult> {
        let format = Self::detect_format(report_path);

        let (metrics, deals) = match format {
            ReportFormat::Xml => self.parse_xml(report_path)?,
            ReportFormat::Html => self.parse_html(report_path)?,
        };

        fs::create_dir_all(output_dir)?;

        let metrics_path = Path::new(output_dir).join("metrics.json");
        self.write_metrics(&metrics, &metrics_path)?;

        Ok(ExtractionResult {
            metrics,
            deals,
            metrics_path,
        })
    }

    pub fn write_deals_to_csv(&self, deals: &[Deal], path: &Path) -> Result<()> {
        self.write_deals_csv(deals, path)
    }

    fn detect_format(path: &str) -> ReportFormat {
        if path.ends_with(".xml") || path.ends_with(".htm.xml") {
            return ReportFormat::Xml;
        }

        if let Ok(file) = fs::read(path) {
            let header = &file[..file.len().min(512)];
            if header.windows(5).any(|w| w == b"<?xml") || header.windows(8).any(|w| w == b"Workbook") {
                return ReportFormat::Xml;
            }
        }

        ReportFormat::Html
    }

    fn parse_html(&self, path: &str) -> Result<(Metrics, Vec<Deal>)> {
        let text = Self::read_text(path)?;
        
        let metrics = Metrics::from_html(&text)
            .ok_or_else(|| anyhow!("No metrics found in HTML report"))?;
        
        let deals = self.parse_deals_html(&text)?;

        Ok((metrics, deals))
    }

    fn parse_xml(&self, path: &str) -> Result<(Metrics, Vec<Deal>)> {
        let text = Self::read_text(path)?;
        
        let metrics = Metrics::from_html(&text)
            .unwrap_or_default();
        
        let deals = self.parse_deals_xml(&text)?;

        Ok((metrics, deals))
    }

    fn parse_deals_html(&self, text: &str) -> Result<Vec<Deal>> {
        let mut deals = Vec::new();

        // (?s) = dotall: makes '.' match '\n' so the regex works on multiline HTML.
        // MT5 HTML column order: Time | Deal | Symbol | Type | Direction |
        //                        Volume | Price | Order | Commission | Swap | Profit | Balance | Comment
        //
        // Strategy: locate the deals table by finding the header <tr> that contains
        // the column names, then parse every subsequent <tr> as a data row.
        // We try two header patterns to handle different MT5 versions/locales.

        let row_re = regex::Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>")
            .map_err(|e| anyhow!("Row regex error: {}", e))?;
        let cell_re = regex::Regex::new(r"(?s)<td[^>]*>(.*?)</td>")
            .map_err(|e| anyhow!("Cell regex error: {}", e))?;

        // Collect all <tr> blocks once, then find the deals header and parse from there.
        let rows: Vec<&str> = row_re.captures_iter(text)
            .filter_map(|cap| cap.get(0).map(|m| m.as_str()))
            .collect();

        // Find the header row index: it must contain both "Deal" and "Symbol" (case-insensitive).
        let header_idx = rows.iter().position(|row| {
            let lower = row.to_lowercase();
            (lower.contains(">deal<") || lower.contains(">deal </")) &&
            (lower.contains(">symbol<") || lower.contains(">symbol </"))
        });

        let start_idx = match header_idx {
            Some(i) => i + 1,
            None => {
                // Fallback: look for any row containing Time+Volume+Profit headers
                let alt = rows.iter().position(|row| {
                    let lower = row.to_lowercase();
                    lower.contains(">time<") && lower.contains(">volume<") && lower.contains(">profit<")
                });
                match alt {
                    Some(i) => i + 1,
                    None => {
                        tracing::warn!("parse_deals_html: no deals table header found");
                        return Ok(deals);
                    }
                }
            }
        };

        for row in &rows[start_idx..] {
            let cells: Vec<String> = cell_re.captures_iter(row)
                .filter_map(|cap| cap.get(1))
                .map(|m| Self::strip_tags(m.as_str()))
                .map(|s| s.replace(',', ""))
                .collect();

            if cells.len() < 3 || cells[0].trim().is_empty() {
                continue;
            }

            // Skip balance/credit operation rows (Type column is index 3 in MT5 HTML)
            let type_cell = cells.get(3).map(|s| s.trim().to_lowercase()).unwrap_or_default();
            if type_cell == "balance" || type_cell == "credit" {
                continue;
            }
            // Also skip sub-header rows that repeat column names
            if type_cell == "type" || cells.get(1).map(|s| s.trim().to_lowercase()).as_deref() == Some("deal") {
                continue;
            }
            // Skip rows with no deal number (e.g. totals row)
            let deal_num = cells.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
            if deal_num.is_empty() {
                continue;
            }

            let deal = Deal {
                time:       cells.get(0).cloned().unwrap_or_default(),
                deal:       deal_num,
                symbol:     cells.get(2).cloned().unwrap_or_default(),
                deal_type:  cells.get(3).cloned().unwrap_or_default(),
                entry:      cells.get(4).cloned().unwrap_or_default(),
                volume:     cells.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                price:      cells.get(6).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                order:      cells.get(7).cloned().unwrap_or_default(),
                commission: cells.get(8).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                swap:       cells.get(9).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                profit:     cells.get(10).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                balance:    cells.get(11).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                comment:    cells.get(12).cloned().unwrap_or_default(),
                magic:      cells.get(13).cloned(),
            };

            deals.push(deal);
        }

        tracing::info!("parse_deals_html: extracted {} deals", deals.len());
        Ok(deals)
    }

    fn parse_deals_xml(&self, text: &str) -> Result<Vec<Deal>> {
        let mut deals = Vec::new();
        let mut header_found = false;
        let mut col_map: HashMap<usize, String> = HashMap::new();

        let row_re = regex::Regex::new(r"<Row[^>]*>(.*?)</Row>")
            .map_err(|e| anyhow!("Regex error: {}", e))?;
        
        let cell_re = regex::Regex::new(r"<Cell[^>]*>.*?<Data[^>]*>(.*?)</Data>.*?</Cell>")
            .map_err(|e| anyhow!("Regex error: {}", e))?;

        for row_caps in row_re.captures_iter(text) {
            let row = row_caps.get(1).map(|m| m.as_str()).unwrap_or("");
            
            let cells: Vec<String> = cell_re.captures_iter(row)
                .filter_map(|cap| cap.get(1))
                .map(|m| Self::strip_tags(m.as_str()).replace(',', ""))
                .collect();

            if !header_found {
                let row_str = cells.join("").to_lowercase();
                if row_str.contains("time") || row_str.contains("type") || row_str.contains("volume") {
                    header_found = true;
                    for (i, h) in cells.iter().enumerate() {
                        let h_lower = h.to_lowercase().trim().to_string();
                        let deal_columns = ["time", "deal", "symbol", "type", "entry", "volume", "price", "order", "commission", "swap", "profit", "balance", "comment"];
                        for col in &deal_columns {
                            if h_lower.contains(col) || col.contains(&h_lower) {
                                col_map.insert(i, col.to_string());
                                break;
                            }
                        }
                    }
                    continue;
                }
            }

            if cells.is_empty() || cells[0].is_empty() {
                continue;
            }

            let mut deal_map: HashMap<String, String> = HashMap::new();
            for (i, val) in cells.iter().enumerate() {
                if let Some(col) = col_map.get(&i) {
                    deal_map.insert(col.clone(), val.clone());
                }
            }

            if !deal_map.is_empty() {
                let deal = Deal {
                    time: deal_map.get("time").cloned().unwrap_or_default(),
                    deal: deal_map.get("deal").cloned().unwrap_or_default(),
                    symbol: deal_map.get("symbol").cloned().unwrap_or_default(),
                    deal_type: deal_map.get("type").cloned().unwrap_or_default(),
                    entry: deal_map.get("entry").cloned().unwrap_or_default(),
                    volume: deal_map.get("volume").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    price: deal_map.get("price").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    order: deal_map.get("order").cloned().unwrap_or_default(),
                    commission: deal_map.get("commission").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    swap: deal_map.get("swap").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    profit: deal_map.get("profit").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    balance: deal_map.get("balance").and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    comment: deal_map.get("comment").cloned().unwrap_or_default(),
                    magic: deal_map.get("magic").cloned(),
                };
                deals.push(deal);
            }
        }

        Ok(deals)
    }

    fn write_metrics(&self, metrics: &Metrics, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(metrics)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    fn write_deals_csv(&self, deals: &[Deal], path: &Path) -> Result<()> {
        let mut file = File::create(path)?;
        writeln!(file, "time,deal,symbol,type,entry,volume,price,order,commission,swap,profit,balance,comment")?;
        
        for deal in deals {
            writeln!(file, "{},{},{},{},{},{},{},{},{},{},{},{},\"{}\"",
                deal.time,
                deal.deal,
                deal.symbol,
                deal.deal_type,
                deal.entry,
                deal.volume,
                deal.price,
                deal.order,
                deal.commission,
                deal.swap,
                deal.profit,
                deal.balance,
                deal.comment.replace('"', "\"\"")
            )?;
        }
        
        Ok(())
    }

    fn read_text(path: &str) -> Result<String> {
        let raw = fs::read(path)?;
        
        if raw.starts_with(&[0xFF, 0xFE]) || raw.starts_with(&[0xFE, 0xFF]) {
            // UTF-16 BOM
            let text = String::from_utf16_lossy(
                raw.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>()
                    .as_slice()
            );
            return Ok(text);
        }
        
        if let Ok(text) = String::from_utf8(raw.clone()) {
            return Ok(text);
        }
        
        Ok(String::from_utf8_lossy(&raw).to_string())
    }

    fn strip_tags(html: &str) -> String {
        let re = regex::Regex::new(r"<[^>]+>").unwrap();
        re.replace_all(html, "").trim().to_string()
    }
}

pub struct ExtractionResult {
    pub metrics: Metrics,
    pub deals: Vec<Deal>,
    #[allow(dead_code)]
    pub metrics_path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum ReportFormat {
    Html,
    Xml,
}
