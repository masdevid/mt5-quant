#!/usr/bin/env python3
"""
extract.py — Single-pass MT5 report parser.

Reads MT5 backtest report (.htm or .htm.xml / SpreadsheetML) and produces:
  - metrics.json  (aggregate summary)
  - deals.csv     (all deals, 13 columns)
  - deals.json    (deals as JSON array)

Usage:
    python3 analytics/extract.py report.htm --output-dir reports/20250101_123456/
"""

import argparse
import csv
import json
import os
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Optional


# MT5 backtest report deals table columns (actual order from HTML):
# Time, Deal, Symbol, Type, Direction, Volume, Price, Order, Commission, Swap, Profit, Balance, Comment
DEAL_COLUMNS = [
    "time", "deal", "symbol", "type", "entry", "volume", "price",
    "order", "commission", "swap", "profit", "balance", "comment"
]


def detect_format(path: str) -> str:
    """Return 'xml' for SpreadsheetML, 'html' for legacy HTML report."""
    if path.endswith('.xml') or path.endswith('.htm.xml'):
        return 'xml'
    # Peek at file header
    with open(path, 'rb') as f:
        header = f.read(512)
    if b'<?xml' in header or b'Workbook' in header:
        return 'xml'
    return 'html'


def read_text(path: str) -> str:
    """Read file, handling UTF-16 (MT5 default) and latin-1 fallback."""
    with open(path, 'rb') as f:
        raw = f.read()
    for encoding in ('utf-16', 'utf-8', 'latin-1'):
        try:
            return raw.decode(encoding)
        except (UnicodeDecodeError, LookupError):
            continue
    return raw.decode('latin-1', errors='replace')


def strip_tags(html: str) -> str:
    return re.sub(r'<[^>]+>', '', html).strip()


# ── HTML parser ───────────────────────────────────────────────────────────────

def parse_html(path: str) -> tuple[dict, list[dict]]:
    text = read_text(path)

    metrics = _parse_metrics_html(text)
    deals = _parse_deals_html(text)
    return metrics, deals


def _parse_metrics_html(text: str) -> dict:
    """Extract aggregate metrics from the summary table."""
    m = {}

    # MT5 report HTML format: MetricLabel:</td>\r\n<td nowrap><b>VALUE</b></td>
    # Helper patterns — values always wrapped in <b>...</b>
    _b = r'[^<]*</td>\s*<td[^>]*>\s*<b>([-\d\s.,]+)</b>'           # plain number
    _b_pct = r'[^<]*</td>\s*<td[^>]*>\s*<b>[^(]*\(([\d.,]+)%\)'    # "abs (pct%)" — capture pct
    patterns = {
        'net_profit':      r'Net\s+Profit' + _b,
        'profit_factor':   r'Profit\s+Factor' + _b,
        'max_dd_pct':      r'Equity\s+Drawdown\s+Maximal' + _b_pct,
        'sharpe_ratio':    r'Sharpe\s+Ratio' + _b,
        'total_trades':    r'Total\s+Trades' + _b,
        'recovery_factor': r'Recovery\s+Factor' + _b,
        'win_rate_pct':    r'Profit\s+Trades\s+\(%' + _b_pct,
        'gross_profit':    r'Gross\s+Profit' + _b,
        'gross_loss':      r'Gross\s+Loss' + _b,
    }

    for key, pattern in patterns.items():
        match = re.search(pattern, text, re.IGNORECASE | re.DOTALL)
        if match:
            val = match.group(1).replace(' ', '').replace(',', '').strip()
            try:
                m[key] = float(val)
            except ValueError:
                pass

    # Trades needs int
    if 'total_trades' in m:
        m['total_trades'] = int(m['total_trades'])

    return m


def _parse_deals_html(text: str) -> list[dict]:
    """Extract deal rows from the deals table."""
    # Find deals section (after "Deals" header)
    deals_section = re.search(
        r'<tr[^>]*>.*?Deal.*?Time.*?Type.*?Direction.*?Volume.*?</tr>(.*)',
        text, re.DOTALL | re.IGNORECASE
    )
    if not deals_section:
        return []

    rows = re.findall(
        r'<tr[^>]*>(.*?)</tr>',
        deals_section.group(1),
        re.DOTALL | re.IGNORECASE
    )

    deals = []
    for row in rows:
        cells = re.findall(r'<td[^>]*>(.*?)</td>', row, re.DOTALL | re.IGNORECASE)
        cells = [strip_tags(c).replace(',', '') for c in cells]

        if len(cells) < 3 or not cells[0]:
            continue
        # Skip balance/deposit/credit rows — 'balance' appears in the Type column (index 3)
        # or sometimes in index 1; check first 5 cells
        if any(c.strip().lower() in ('balance', 'credit') for c in cells[:5]):
            continue

        deal = {}
        for i, col in enumerate(DEAL_COLUMNS):
            deal[col] = cells[i] if i < len(cells) else ''
        deals.append(deal)

    return deals


# ── XML parser (SpreadsheetML) ─────────────────────────────────────────────────

def parse_xml(path: str) -> tuple[dict, list[dict]]:
    """Parse MT5 SpreadsheetML optimization/report XML."""
    tree = ET.parse(path)
    root = tree.getroot()

    # Namespace handling — MT5 XML uses Excel namespace
    ns = {}
    ns_match = re.match(r'\{([^}]+)\}', root.tag)
    if ns_match:
        ns['ss'] = ns_match.group(1)

    def tag(name):
        return f"{{{ns['ss']}}}{name}" if ns else name

    metrics = {}
    deals = []
    in_deals_sheet = False

    for sheet in root.iter(tag('Worksheet')):
        sheet_name = sheet.get(f"{{{ns['ss']}}}Name" if ns else 'Name', '')

        if 'result' in sheet_name.lower() or 'report' in sheet_name.lower():
            metrics = _parse_metrics_xml(sheet, tag)
        elif 'deal' in sheet_name.lower() or 'trade' in sheet_name.lower():
            deals = _parse_deals_xml(sheet, tag)
        elif sheet_name == '':
            # Unnamed sheet — check if it has deal-like structure
            rows = list(sheet.iter(tag('Row')))
            if len(rows) > 5:
                # Try to parse as deals
                candidate = _parse_deals_xml(sheet, tag)
                if candidate:
                    deals = candidate

    return metrics, deals


def _cell_value(cell, tag) -> str:
    data = cell.find(tag('Data'))
    return data.text.strip() if data is not None and data.text else ''


def _parse_metrics_xml(sheet, tag) -> dict:
    m = {}
    for row in sheet.iter(tag('Row')):
        cells = [_cell_value(c, tag) for c in row.iter(tag('Cell'))]
        if len(cells) < 2:
            continue
        key = cells[0].lower()
        val = cells[1].replace(',', '').strip()
        try:
            fval = float(val)
            if 'net profit' in key or 'net_profit' in key:
                m['net_profit'] = fval
            elif 'profit factor' in key:
                m['profit_factor'] = fval
            elif 'drawdown' in key and '%' in cells[1]:
                m['max_dd_pct'] = fval
            elif 'sharpe' in key:
                m['sharpe_ratio'] = fval
            elif 'total trades' in key:
                m['total_trades'] = int(fval)
        except (ValueError, AttributeError):
            pass
    return m


def _parse_deals_xml(sheet, tag) -> list[dict]:
    deals = []
    header_found = False
    col_map = {}

    for row in sheet.iter(tag('Row')):
        cells = [_cell_value(c, tag) for c in row.iter(tag('Cell'))]

        if not header_found:
            # Detect header row
            if any(h in str(cells).lower() for h in ('time', 'type', 'volume', 'profit')):
                header_found = True
                for i, h in enumerate(cells):
                    h_lower = h.lower().strip()
                    for col in DEAL_COLUMNS:
                        if col in h_lower or h_lower in col:
                            col_map[i] = col
                            break
            continue

        if not cells or not cells[0]:
            continue

        deal = {}
        for i, val in enumerate(cells):
            col = col_map.get(i)
            if col:
                deal[col] = val.replace(',', '')

        if deal:
            deals.append(deal)

    return deals


# ── Writer ────────────────────────────────────────────────────────────────────

def write_outputs(metrics: dict, deals: list[dict], output_dir: str) -> dict:
    os.makedirs(output_dir, exist_ok=True)

    metrics_path = os.path.join(output_dir, 'metrics.json')
    deals_csv_path = os.path.join(output_dir, 'deals.csv')
    deals_json_path = os.path.join(output_dir, 'deals.json')

    with open(metrics_path, 'w') as f:
        json.dump(metrics, f, indent=2)

    with open(deals_json_path, 'w') as f:
        json.dump(deals, f, indent=2)

    if deals:
        all_keys = DEAL_COLUMNS
        with open(deals_csv_path, 'w', newline='') as f:
            writer = csv.DictWriter(f, fieldnames=all_keys, extrasaction='ignore')
            writer.writeheader()
            writer.writerows(deals)
    else:
        # Write empty CSV with headers
        with open(deals_csv_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(DEAL_COLUMNS)

    return {
        'metrics': metrics_path,
        'deals_csv': deals_csv_path,
        'deals_json': deals_json_path,
    }


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description='Extract MT5 backtest report')
    parser.add_argument('report', help='Path to report.htm or report.htm.xml')
    parser.add_argument('--output-dir', default='.', help='Output directory')
    parser.add_argument('--stdout', action='store_true',
                        help='Print metrics JSON to stdout instead of writing files')
    args = parser.parse_args()

    fmt = detect_format(args.report)

    if fmt == 'xml':
        metrics, deals = parse_xml(args.report)
    else:
        metrics, deals = parse_html(args.report)

    if not metrics:
        print(f"WARNING: No aggregate metrics found in report", file=sys.stderr)
    if not deals:
        print(f"WARNING: No deals found in report (check date range and symbol)", file=sys.stderr)

    if args.stdout:
        json.dump({'metrics': metrics, 'deals_count': len(deals)}, sys.stdout, indent=2)
        print()
        return

    paths = write_outputs(metrics, deals, args.output_dir)

    print(f"Extracted: {len(deals)} deals, {len(metrics)} metrics")
    for name, path in paths.items():
        print(f"  {name}: {path}")


if __name__ == '__main__':
    main()
