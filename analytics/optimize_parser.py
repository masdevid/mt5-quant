#!/usr/bin/env python3
"""
optimize_parser.py — Parse MT5 genetic optimization results.

Handles both HTML (.htm) and SpreadsheetML XML (.htm.xml) formats.

Usage:
    python3 analytics/optimize_parser.py --job opt_20250619_143022
    python3 analytics/optimize_parser.py --file reports/opt_dir/optimization.htm
    python3 analytics/optimize_parser.py --file report.htm.xml --top 30 --sort profit
"""

import argparse
import json
import os
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT_DIR = Path(__file__).parent.parent


def find_report(job_id: str) -> str:
    """Locate optimization report from job metadata."""
    jobs_dir = ROOT_DIR / '.mt5mcp_jobs'
    meta_path = jobs_dir / f'{job_id}.json'

    if not meta_path.exists():
        raise FileNotFoundError(f"Job not found: {job_id}. Check .mt5mcp_jobs/")

    with open(meta_path) as f:
        meta = json.load(f)

    wine_prefix = meta.get('wine_prefix', '')
    base = os.path.join(wine_prefix, 'drive_c', 'mt5mcp_opt_report')

    for ext in ('.htm', '.htm.xml', '.html'):
        candidate = base + ext
        if os.path.exists(candidate):
            return candidate

    raise FileNotFoundError(
        f"Optimization report not found. Expected: {base}.htm or {base}.htm.xml\n"
        f"Is MT5 optimization still running? Check log: {meta.get('log_file', '')}"
    )


def detect_format(path: str) -> str:
    if path.endswith('.xml') or path.endswith('.htm.xml'):
        return 'xml'
    with open(path, 'rb') as f:
        header = f.read(512)
    if b'<?xml' in header or b'Workbook' in header:
        return 'xml'
    return 'html'


def read_text(path: str) -> str:
    with open(path, 'rb') as f:
        raw = f.read()
    for enc in ('utf-16', 'utf-8', 'latin-1'):
        try:
            return raw.decode(enc)
        except (UnicodeDecodeError, LookupError):
            continue
    return raw.decode('latin-1', errors='replace')


# ── HTML parser ───────────────────────────────────────────────────────────────

def parse_html(path: str) -> list[dict]:
    text = read_text(path)

    rows = re.findall(r'<tr[^>]*>(.*?)</tr>', text, re.DOTALL | re.IGNORECASE)
    results = []
    headers = []

    for row in rows:
        cells = re.findall(r'<t[dh][^>]*>(.*?)</t[dh]>', row, re.DOTALL | re.IGNORECASE)
        cells = [re.sub(r'<[^>]+>', '', c).strip().replace(',', '') for c in cells]

        if not cells:
            continue

        # Header row detection
        if not headers and cells[0].lower() in ('pass', '#', 'result', 'run'):
            headers = cells
            continue

        # Data row: first cell is pass number (digit)
        if headers and cells[0].isdigit():
            row_data = dict(zip(headers, cells))
            results.append(row_data)
        elif not headers and cells[0].isdigit() and len(cells) > 5:
            # No header — use positional mapping (common MT5 layout)
            results.append(_positional_row(cells))

    return results


def _positional_row(cells: list[str]) -> dict:
    """Map cells by position for headerless optimization tables."""
    # MT5 optimization table columns (typical order):
    # Pass | Profit | Expected Payoff | Profit Factor | Recovery Factor | Sharpe | Custom | DD% | Trades | ...params
    pos_names = ['pass', 'profit', 'expected_payoff', 'profit_factor',
                 'recovery_factor', 'sharpe_ratio', 'custom', 'max_dd_pct', 'total_trades']
    row = {}
    for i, name in enumerate(pos_names):
        if i < len(cells):
            row[name] = cells[i]
    # Remaining are parameters
    row['_params_raw'] = cells[len(pos_names):]
    return row


# ── XML parser ────────────────────────────────────────────────────────────────

def parse_xml(path: str) -> list[dict]:
    tree = ET.parse(path)
    root = tree.getroot()

    ns = {}
    ns_match = re.match(r'\{([^}]+)\}', root.tag)
    if ns_match:
        ns['ss'] = ns_match.group(1)

    def tag(name):
        return f"{{{ns['ss']}}}{name}" if ns else name

    def cell_val(cell):
        data = cell.find(tag('Data'))
        return data.text.strip() if data is not None and data.text else ''

    results = []
    headers = []

    for sheet in root.iter(tag('Worksheet')):
        for row in sheet.iter(tag('Row')):
            cells = [cell_val(c) for c in row.iter(tag('Cell'))]
            cells = [c.replace(',', '').strip() for c in cells]

            if not cells:
                continue

            if not headers:
                if any(h.lower() in ('pass', 'result', 'profit') for h in cells):
                    headers = cells
                    continue

            if cells[0].isdigit():
                if headers:
                    row_data = {}
                    for i, h in enumerate(headers):
                        row_data[h.lower().replace(' ', '_')] = cells[i] if i < len(cells) else ''
                    results.append(row_data)
                else:
                    results.append(_positional_row(cells))

    return results


# ── Normalizer ────────────────────────────────────────────────────────────────

def normalize(raw_results: list[dict]) -> list[dict]:
    """Convert raw parsed rows to typed dicts with consistent keys."""
    normalized = []

    for r in raw_results:
        def fget(keys, default=0.0):
            for k in keys:
                for rk, rv in r.items():
                    if k in rk.lower().replace(' ', '_'):
                        try:
                            return float(rv)
                        except (ValueError, TypeError):
                            pass
            return default

        def iget(keys, default=0):
            v = fget(keys, default)
            return int(v)

        # Extract known fields
        entry = {
            'pass': iget(['pass', '#']),
            'net_profit': fget(['profit', 'net_profit']),
            'profit_factor': fget(['profit_factor']),
            'max_dd_pct': fget(['dd', 'drawdown']),
            'total_trades': iget(['trades']),
            'sharpe_ratio': fget(['sharpe']),
            'recovery_factor': fget(['recovery']),
        }

        # Remaining keys are parameters
        known_keys = {'pass', 'profit', 'net_profit', 'profit_factor', 'expected_payoff',
                      'dd', 'drawdown', 'max_dd_pct', 'trades', 'total_trades',
                      'sharpe', 'sharpe_ratio', 'recovery', 'recovery_factor',
                      'custom', '#', '_params_raw'}

        params = {}
        for k, v in r.items():
            if not any(kw in k.lower() for kw in known_keys):
                try:
                    params[k] = float(v)
                except (ValueError, TypeError):
                    params[k] = v

        entry['params'] = params
        normalized.append(entry)

    return normalized


# ── Convergence analysis ──────────────────────────────────────────────────────

def convergence_analysis(results: list[dict], top_n: int = 10) -> dict:
    top = results[:top_n]
    if not top:
        return {}

    all_param_keys = set()
    for r in top:
        all_param_keys.update(r.get('params', {}).keys())

    strong = {}   # Same value across all top-N
    uncertain = []  # Varies

    for key in all_param_keys:
        values = set()
        for r in top:
            v = r.get('params', {}).get(key)
            if v is not None:
                values.add(v)
        if len(values) == 1:
            strong[key] = list(values)[0]
        else:
            uncertain.append(key)

    return {
        'top_n_agreement': strong,
        'high_variance_params': uncertain,
    }


# ── Display ───────────────────────────────────────────────────────────────────

def display_results(results: list[dict], top_n: int, dd_threshold: float, conv: dict):
    print(f"\nTotal passes: {len(results)}")
    print(f"Showing top {min(top_n, len(results))} by profit:\n")

    print(f"{'Rank':<5} {'Profit':>10} {'PF':>6} {'DD%':>6} {'Sharpe':>7} {'Trades':>7}  Params")
    print("─" * 80)

    for i, r in enumerate(results[:top_n], 1):
        dd = r['max_dd_pct']
        risk_flag = ' ⚠' if dd > dd_threshold else ''
        params_str = '  '.join(f"{k}={v}" for k, v in list(r.get('params', {}).items())[:4])
        print(
            f"#{i:<4} ${r['net_profit']:>9,.2f} "
            f"{r['profit_factor']:>5.2f} "
            f"{dd:>5.2f}%"
            f"{risk_flag} "
            f"{r['sharpe_ratio']:>6.2f} "
            f"{r['total_trades']:>7}  "
            f"{params_str}"
        )

    if conv:
        print(f"\nConvergence (top-{min(top_n, len(results))} agreement):")
        if conv.get('top_n_agreement'):
            print("  Stable params:", ', '.join(f"{k}={v}" for k, v in conv['top_n_agreement'].items()))
        if conv.get('high_variance_params'):
            print("  Uncertain params:", ', '.join(conv['high_variance_params']))


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description='Parse MT5 optimization results')
    parser.add_argument('--job', help='Job ID from optimize.sh output')
    parser.add_argument('--file', help='Direct path to optimization.htm or .htm.xml')
    parser.add_argument('--top', type=int, default=20, help='Show top N results')
    parser.add_argument('--sort', choices=['profit', 'profit_factor', 'sharpe'],
                        default='profit', help='Sort metric')
    parser.add_argument('--dd-threshold', type=float, default=20.0,
                        help='Flag DD above this % as high-risk')
    parser.add_argument('--output', help='Save results as JSON')
    args = parser.parse_args()

    # Locate report
    if args.file:
        report_path = args.file
    elif args.job:
        try:
            report_path = find_report(args.job)
        except FileNotFoundError as e:
            print(f"ERROR: {e}", file=sys.stderr)
            sys.exit(1)
    else:
        print("ERROR: Provide --job or --file", file=sys.stderr)
        sys.exit(1)

    if not os.path.exists(report_path):
        print(f"ERROR: Report not found: {report_path}", file=sys.stderr)
        sys.exit(1)

    # Parse
    fmt = detect_format(report_path)
    if fmt == 'xml':
        raw = parse_xml(report_path)
    else:
        raw = parse_html(report_path)

    results = normalize(raw)

    if not results:
        print("ERROR: No optimization passes found in report.", file=sys.stderr)
        sys.exit(1)

    # Sort
    sort_key = {
        'profit': 'net_profit',
        'profit_factor': 'profit_factor',
        'sharpe': 'sharpe_ratio',
    }[args.sort]
    results.sort(key=lambda r: r.get(sort_key, 0), reverse=True)

    # Convergence analysis
    conv = convergence_analysis(results, top_n=10)

    # Display
    display_results(results, args.top, args.dd_threshold, conv)

    # Optional JSON output
    if args.output:
        output = {
            'total_passes': len(results),
            'results': results[:args.top],
            'convergence': conv,
        }
        with open(args.output, 'w') as f:
            json.dump(output, f, indent=2)
        print(f"\nSaved: {args.output}")


if __name__ == '__main__':
    main()
