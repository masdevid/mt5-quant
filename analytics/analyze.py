#!/usr/bin/env python3
"""
analyze.py — Deal-level backtest analysis engine.

Strategy-agnostic core + strategy-specific profiles.

Usage:
    # Generic (works for any EA — no hardcoded keyword assumptions)
    python3 analytics/analyze.py generic deals.csv --output-dir DIR

    # Strategy-specific presets
    python3 analytics/analyze.py grid    deals.csv --output-dir DIR
    python3 analytics/analyze.py scalper deals.csv --output-dir DIR
    python3 analytics/analyze.py trend   deals.csv --output-dir DIR
    python3 analytics/analyze.py hedge   deals.csv --output-dir DIR

    # Legacy (no subcommand) — defaults to 'grid' for backward compatibility
    python3 analytics/analyze.py deals.csv --output-dir DIR

    # Flags
    --deep      Add hourly_pnl and volume_profile
    --stdout    Print JSON to stdout instead of writing analysis.json

Entry points (after pip install -e .):
    mt5-analyze         generic analysis
    mt5-analyze-grid    grid / martingale
    mt5-analyze-scalper scalper
    mt5-analyze-trend   trend following
    mt5-analyze-hedge   hedging
"""

import argparse
import csv
import json
import os
import re
import sys
from collections import defaultdict
from datetime import datetime
from typing import Optional


# ── Strategy profiles ─────────────────────────────────────────────────────────
#
# Each profile is a plain dict with:
#   name             – human-readable label
#   depth_re         – regex to extract depth number from comment (None = no depth tracking)
#   exit_keywords    – {reason: [keywords]} for comment-based exit classification
#   dd_cause_keywords– {cause: [keywords]} for comment-based DD cause classification
#   cycle_group_by   – 'magic' | 'magic+direction' | None
#   cycle_gap_min    – minutes between opens that marks a new cycle boundary
#
# When exit_keywords is empty, classification falls back to profit-sign (tp/sl).
# When dd_cause_keywords is empty, dd_events cause = 'unknown'.

PROFILES: dict[str, dict] = {
    'generic': {
        'name': 'Generic',
        'depth_re': None,
        'exit_keywords': {},
        'dd_cause_keywords': {},
        'cycle_group_by': 'magic',
        'cycle_gap_min': 60,
    },
    'grid': {
        'name': 'Grid / Martingale',
        'depth_re': r'[Ll]ayer\s*#?(\d+)',
        'exit_keywords': {
            'locking':  ['locking', 'lock'],
            'cutloss':  ['cutloss', 'cut loss', 'cut_loss'],
            'zombie':   ['zombie'],
            'timeout':  ['timeout', 'time out', 'time_out'],
        },
        'dd_cause_keywords': {
            'locking_cascade': ['locking', 'lock'],
            'cutloss':         ['cutloss', 'cut'],
            'zombie_exit':     ['zombie'],
            'spike_entry':     ['spike'],
        },
        'cycle_group_by': 'magic+direction',
        'cycle_gap_min': 60,
    },
    'scalper': {
        'name': 'Scalper',
        'depth_re': None,
        'exit_keywords': {
            'tp':       ['tp', 'take profit', 'target'],
            'sl':       ['sl', 'stop loss', 'stoploss'],
            'manual':   ['manual', 'close'],
            'trailing': ['trailing', 'trail'],
        },
        'dd_cause_keywords': {
            'stop_loss':    ['sl', 'stop'],
            'manual_close': ['manual', 'close'],
        },
        'cycle_group_by': 'magic',
        'cycle_gap_min': 10,
    },
    'trend': {
        'name': 'Trend Following',
        'depth_re': None,
        'exit_keywords': {
            # More specific patterns must come before general ones to avoid substring
            # false-positives (e.g. 'stop' inside 'breakeven stop' / 'trailing stop').
            'breakeven': ['breakeven', 'break even', 'be stop'],
            'trailing':  ['trailing', 'trail'],
            'partial':   ['partial', 'scale out'],
            'tp':        ['tp', 'target', 'take profit'],
            'sl':        ['sl', 'stop loss', 'stoploss'],
        },
        'dd_cause_keywords': {
            'whipsaw':        ['stop loss', 'stoploss'],
            'trailing_stop':  ['trailing'],
            'breakeven_stop': ['breakeven', 'be stop'],
        },
        'cycle_group_by': 'magic',
        'cycle_gap_min': 240,
    },
    'hedge': {
        'name': 'Hedging',
        'depth_re': None,
        'exit_keywords': {
            'tp':        ['tp'],
            'sl':        ['sl'],
            'net_close': ['net', 'hedge close', 'hedge_close'],
            'partial':   ['partial', 'reduce'],
        },
        'dd_cause_keywords': {
            'hedge_unwind':      ['net', 'hedge'],
            'correlation_break': ['sl', 'stop'],
        },
        'cycle_group_by': 'magic+direction',
        'cycle_gap_min': 120,
    },
}


# ── Utility ───────────────────────────────────────────────────────────────────

def _parse_dt(time_str: str) -> Optional[datetime]:
    """Parse MT5 time string ('2025.01.10 09:30:00') to datetime."""
    if not time_str:
        return None
    s = time_str.strip()
    for length, fmt in [(19, '%Y.%m.%d %H:%M:%S'), (19, '%Y-%m-%d %H:%M:%S'),
                        (10, '%Y.%m.%d'), (10, '%Y-%m-%d')]:
        try:
            return datetime.strptime(s[:length], fmt)
        except (ValueError, IndexError):
            continue
    return None


def _extract_depth(comment: str, depth_re: Optional[str]) -> int:
    """Extract a numeric depth from comment using the given regex. Returns 0 if not matched."""
    if not depth_re or not comment:
        return 0
    match = re.search(depth_re, comment)
    return int(match.group(1)) if match else 0


def _layer_from_comment(comment: str) -> int:
    """Grid-specific: extract Layer #N number. Kept for backward compatibility."""
    return _extract_depth(comment, PROFILES['grid']['depth_re'])


def _classify_exit(comment: str, profit: float,
                   profile: Optional[dict] = None) -> str:
    """
    Classify exit reason from deal comment.

    If profile is provided, uses its exit_keywords dict.
    If profile is None, falls back to the grid profile keywords (backward compat).
    If no keywords match, returns 'tp' (profit > 0) or 'sl' (profit <= 0).
    """
    kw_map = (profile or PROFILES['grid'])['exit_keywords']
    c = comment.lower()
    for reason, keywords in kw_map.items():
        if any(kw in c for kw in keywords):
            return reason
    return 'tp' if profit > 0 else 'sl'


def _classify_dd_cause(comment: str, profile: dict) -> str:
    """Classify DD event cause from comment using profile's dd_cause_keywords."""
    c = comment.lower()
    for cause, keywords in profile.get('dd_cause_keywords', {}).items():
        if any(kw in c for kw in keywords):
            return cause
    return 'unknown'


def _lot_tier(vol: float) -> str:
    if vol <= 0.01:  return '0.01'
    if vol <= 0.04:  return '0.02-0.04'
    if vol <= 0.09:  return '0.05-0.09'
    if vol <= 0.49:  return '0.10-0.49'
    if vol <= 0.99:  return '0.50-0.99'
    return '1.00+'


def _session_for_hour(hour: int) -> str:
    if 13 <= hour < 17: return 'london_ny_overlap'
    if  8 <= hour < 17: return 'london'
    if 17 <= hour < 22: return 'new_york'
    if  0 <= hour <  8: return 'asian'
    return 'off_hours'


# ── Loaders ───────────────────────────────────────────────────────────────────

def load_deals(csv_path: str) -> list[dict]:
    deals = []
    with open(csv_path, newline='', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            row = {k: (v or '').strip() for k, v in row.items()}
            for field in ('profit', 'volume', 'price', 'balance'):
                try:
                    row[field] = float(row.get(field, 0).replace(',', '') or 0)
                except ValueError:
                    row[field] = 0.0
            deals.append(row)
    return deals


def load_metrics(metrics_path: str) -> dict:
    if not os.path.exists(metrics_path):
        return {}
    with open(metrics_path) as f:
        return json.load(f)


# ── Generic analytics (strategy-agnostic) ────────────────────────────────────

def monthly_pnl(deals: list[dict]) -> list[dict]:
    monthly: dict[str, dict] = defaultdict(lambda: {'pnl': 0.0, 'trades': 0})
    for deal in deals:
        time_str = deal.get('time', '')
        profit = deal.get('profit', 0.0)
        entry = deal.get('entry', '').lower()
        if 'out' not in entry and entry != '':
            continue
        if not time_str or profit == 0.0:
            continue
        try:
            dt = datetime.strptime(time_str[:10].replace('.', '-'), '%Y-%m-%d')
            month = dt.strftime('%Y-%m')
        except (ValueError, IndexError):
            continue
        monthly[month]['pnl'] += profit
        monthly[month]['trades'] += 1

    return [
        {'month': m, 'pnl': round(d['pnl'], 2), 'trades': d['trades'], 'green': d['pnl'] >= 0}
        for m in sorted(monthly)
        for d in [monthly[m]]
    ]


def reconstruct_dd_events(deals: list[dict], metrics: dict,
                          profile: Optional[dict] = None) -> list[dict]:
    """Walk deals chronologically, reconstruct drawdown events. Cause classified by profile."""
    if not deals:
        return []

    _profile = profile or PROFILES['grid']
    balance_curve = []
    peak_balance = 0.0
    initial_balance = None

    for deal in deals:
        balance = deal.get('balance', 0.0)
        if balance > 0:
            if initial_balance is None:
                initial_balance = balance
            peak_balance = max(peak_balance, balance)
            dd_pct = (peak_balance - balance) / peak_balance * 100 if peak_balance > 0 else 0
            balance_curve.append({
                'time':    deal.get('time', ''),
                'balance': balance,
                'dd_pct':  round(dd_pct, 3),
                'profit':  deal.get('profit', 0.0),
                'comment': deal.get('comment', ''),
            })

    if not balance_curve:
        return []

    events, in_dd, dd_start_idx = [], False, None
    threshold = 1.0

    for i, point in enumerate(balance_curve):
        if not in_dd and point['dd_pct'] > threshold:
            in_dd, dd_start_idx = True, i
        elif in_dd and point['dd_pct'] < threshold:
            peak_idx = max(range(dd_start_idx, i + 1),
                           key=lambda x: balance_curve[x]['dd_pct'])
            event = _build_dd_event(balance_curve, dd_start_idx, peak_idx, i, _profile)
            if event['peak_dd_pct'] > 1.0:
                events.append(event)
            in_dd, dd_start_idx = False, None

    if in_dd and dd_start_idx is not None:
        peak_idx = max(range(dd_start_idx, len(balance_curve)),
                       key=lambda x: balance_curve[x]['dd_pct'])
        event = _build_dd_event(balance_curve, dd_start_idx, peak_idx, None, _profile)
        if event['peak_dd_pct'] > 1.0:
            events.append(event)

    events.sort(key=lambda e: e['peak_dd_pct'], reverse=True)
    return events[:10]


def _build_dd_event(curve: list[dict], start_idx: int, peak_idx: int,
                    recovery_idx: Optional[int], profile: dict) -> dict:
    start = curve[start_idx]
    peak  = curve[peak_idx]

    event: dict = {
        'peak_dd_pct': round(peak['dd_pct'], 2),
        'start_date':  start['time'][:10].replace('.', '-') if start['time'] else '',
        'end_date':    peak['time'][:10].replace('.', '-')  if peak['time']  else '',
        'cause':       _classify_dd_cause(peak.get('comment', ''), profile),
    }

    if recovery_idx is not None:
        rec = curve[recovery_idx]
        event['recovery_date'] = rec['time'][:10].replace('.', '-') if rec['time'] else None
        try:
            s_dt = datetime.strptime(event['start_date'], '%Y-%m-%d')
            r_dt = datetime.strptime(event['recovery_date'], '%Y-%m-%d')
            event['recovery_days'] = (r_dt - s_dt).days
        except (ValueError, TypeError):
            event['recovery_days'] = None
    else:
        event['recovery_date'] = None
        event['recovery_days'] = None

    try:
        s_dt = datetime.strptime(event['start_date'], '%Y-%m-%d')
        e_dt = datetime.strptime(event['end_date'],   '%Y-%m-%d')
        event['duration_days'] = (e_dt - s_dt).days
    except ValueError:
        event['duration_days'] = 0

    return event


def top_losses(deals: list[dict], n: int = 10) -> list[dict]:
    losses = [
        {
            'date':               deal.get('time', '')[:10].replace('.', '-'),
            'loss_usd':           round(deal['profit'], 2),
            'comment':            deal.get('comment', ''),
            'grid_depth_at_close': _layer_from_comment(deal.get('comment', '')),
            'volume':             deal.get('volume', 0.0),
        }
        for deal in deals
        if deal.get('profit', 0.0) < 0
    ]
    losses.sort(key=lambda x: x['loss_usd'])
    return losses[:n]


def loss_sequences(deals: list[dict]) -> list[dict]:
    closed = [d for d in deals
              if 'out' in d.get('entry', '').lower() and d.get('profit', 0) != 0]
    if not closed:
        return []

    sequences, current_seq = [], []
    for deal in closed:
        if deal['profit'] < 0:
            current_seq.append(deal)
        else:
            if len(current_seq) >= 2:
                total = sum(d['profit'] for d in current_seq)
                sequences.append({
                    'length':     len(current_seq),
                    'total_loss': round(total, 2),
                    'start':      current_seq[0].get('time', '')[:10].replace('.', '-'),
                    'end':        current_seq[-1].get('time', '')[:10].replace('.', '-'),
                })
            current_seq = []

    if len(current_seq) >= 2:
        total = sum(d['profit'] for d in current_seq)
        sequences.append({
            'length':     len(current_seq),
            'total_loss': round(total, 2),
            'start':      current_seq[0].get('time', '')[:10].replace('.', '-'),
            'end':        current_seq[-1].get('time', '')[:10].replace('.', '-'),
        })

    sequences.sort(key=lambda x: x['total_loss'])
    return sequences[:5]


def position_pairs(deals: list[dict]) -> list[dict]:
    """Match in/out deals by order ticket → hold time + depth at close."""
    open_pos: dict[str, dict] = {}
    pairs = []

    for deal in deals:
        order = deal.get('order', '')
        entry = deal.get('entry', '').lower()

        if 'in' in entry and 'out' not in entry:
            open_pos[order] = deal

        elif 'out' in entry:
            profit = deal.get('profit', 0.0)
            if profit == 0.0:
                continue

            in_deal = open_pos.pop(order, None)
            dt_out  = _parse_dt(deal.get('time', ''))
            comment = deal.get('comment', '')

            hold_minutes = None
            if in_deal and dt_out:
                dt_in = _parse_dt(in_deal.get('time', ''))
                if dt_in:
                    hold_minutes = round((dt_out - dt_in).total_seconds() / 60, 1)

            pairs.append({
                'time':         deal.get('time', ''),
                'type':         deal.get('type', ''),
                'profit':       profit,
                'volume':       deal.get('volume', 0.0),
                'layer':        _layer_from_comment(comment),
                'hold_minutes': hold_minutes,
                'comment':      comment,
                'magic':        deal.get('magic', ''),
                'order':        order,
            })

    return pairs


def direction_bias(deals: list[dict]) -> dict:
    stats: dict[str, dict] = {
        'buy':  {'trades': 0, 'wins': 0, 'total_pnl': 0.0},
        'sell': {'trades': 0, 'wins': 0, 'total_pnl': 0.0},
    }
    for deal in deals:
        if 'out' not in deal.get('entry', '').lower():
            continue
        profit = deal.get('profit', 0.0)
        if profit == 0.0:
            continue
        d = deal.get('type', '').lower()
        if d not in stats:
            continue
        stats[d]['trades'] += 1
        stats[d]['total_pnl'] += profit
        if profit > 0:
            stats[d]['wins'] += 1

    return {
        d: {
            'trades':    s['trades'],
            'win_rate':  round(s['wins'] / s['trades'] * 100, 1),
            'total_pnl': round(s['total_pnl'], 2),
            'avg_pnl':   round(s['total_pnl'] / s['trades'], 2),
        }
        for d, s in stats.items() if s['trades'] > 0
    }


def streak_analysis(deals: list[dict]) -> dict:
    closed = [d for d in deals
              if 'out' in d.get('entry', '').lower() and d.get('profit', 0.0) != 0.0]
    if not closed:
        return {}

    max_win_streak = max_loss_streak = cur_win = cur_loss = 0
    max_win_start = max_win_end = max_loss_start = max_loss_end = ''
    win_run_start = loss_run_start = ''

    for deal in closed:
        profit = deal['profit']
        t = deal.get('time', '')[:10].replace('.', '-')
        if profit > 0:
            if cur_win == 0:
                win_run_start = t
            cur_win  += 1
            cur_loss  = 0
            if cur_win > max_win_streak:
                max_win_streak = cur_win
                max_win_start  = win_run_start
                max_win_end    = t
        else:
            if cur_loss == 0:
                loss_run_start = t
            cur_loss += 1
            cur_win   = 0
            if cur_loss > max_loss_streak:
                max_loss_streak = cur_loss
                max_loss_start  = loss_run_start
                max_loss_end    = t

    last = closed[-1]
    return {
        'max_win_streak':    max_win_streak,
        'max_win_start':     max_win_start,
        'max_win_end':       max_win_end,
        'max_loss_streak':   max_loss_streak,
        'max_loss_start':    max_loss_start,
        'max_loss_end':      max_loss_end,
        'current_streak':    cur_win if last['profit'] > 0 else cur_loss,
        'current_streak_type': 'win' if last['profit'] > 0 else 'loss',
    }


def session_breakdown(deals: list[dict]) -> dict:
    """P/L by trading session (UTC hour-based)."""
    sessions: dict = defaultdict(lambda: {'trades': 0, 'wins': 0, 'total_pnl': 0.0})
    for deal in deals:
        if 'out' not in deal.get('entry', '').lower():
            continue
        profit = deal.get('profit', 0.0)
        if profit == 0.0:
            continue
        dt = _parse_dt(deal.get('time', ''))
        if not dt:
            continue
        s = _session_for_hour(dt.hour)
        sessions[s]['trades']    += 1
        sessions[s]['total_pnl'] += profit
        if profit > 0:
            sessions[s]['wins'] += 1

    return {
        s: {
            'trades':    d['trades'],
            'win_rate':  round(d['wins'] / d['trades'] * 100, 1) if d['trades'] > 0 else 0.0,
            'total_pnl': round(d['total_pnl'], 2),
        }
        for s, d in sessions.items()
    }


def weekday_pnl(deals: list[dict]) -> list[dict]:
    DAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']
    by_day: dict = defaultdict(lambda: {'pnl': 0.0, 'trades': 0, 'wins': 0})
    for deal in deals:
        if 'out' not in deal.get('entry', '').lower():
            continue
        profit = deal.get('profit', 0.0)
        if profit == 0.0:
            continue
        dt = _parse_dt(deal.get('time', ''))
        if not dt:
            continue
        by_day[dt.weekday()]['pnl']    += profit
        by_day[dt.weekday()]['trades'] += 1
        if profit > 0:
            by_day[dt.weekday()]['wins'] += 1

    return [
        {
            'day':      DAY_NAMES[day],
            'pnl':      round(s['pnl'], 2),
            'trades':   s['trades'],
            'win_rate': round(s['wins'] / s['trades'] * 100, 1) if s['trades'] > 0 else 0.0,
        }
        for day in sorted(by_day)
        for s in [by_day[day]]
    ]


def hourly_pnl(deals: list[dict]) -> list[dict]:
    """P/L by close hour (0–23). Intended for --deep mode."""
    by_hour: dict = defaultdict(lambda: {'pnl': 0.0, 'trades': 0, 'wins': 0})
    for deal in deals:
        if 'out' not in deal.get('entry', '').lower():
            continue
        profit = deal.get('profit', 0.0)
        if profit == 0.0:
            continue
        dt = _parse_dt(deal.get('time', ''))
        if not dt:
            continue
        by_hour[dt.hour]['pnl']    += profit
        by_hour[dt.hour]['trades'] += 1
        if profit > 0:
            by_hour[dt.hour]['wins'] += 1

    return [
        {
            'hour':     h,
            'pnl':      round(s['pnl'], 2),
            'trades':   s['trades'],
            'win_rate': round(s['wins'] / s['trades'] * 100, 1) if s['trades'] > 0 else 0.0,
        }
        for h in sorted(by_hour)
        for s in [by_hour[h]]
    ]


def concurrent_peak(deals: list[dict]) -> dict:
    """Peak number of simultaneously open positions."""
    events = []
    for deal in deals:
        entry = deal.get('entry', '').lower()
        dt    = _parse_dt(deal.get('time', ''))
        if not dt:
            continue
        if 'in' in entry and 'out' not in entry:
            events.append((dt,  1, deal))
        elif 'out' in entry:
            events.append((dt, -1, deal))

    events.sort(key=lambda x: x[0])
    count = peak = 0
    peak_time = ''
    for dt, delta, deal in events:
        count = max(0, count + delta)
        if count > peak:
            peak      = count
            peak_time = deal.get('time', '')

    return {'peak_open': peak, 'peak_time': peak_time}


def volume_profile(deals: list[dict]) -> list[dict]:
    """P/L breakdown by lot size tier. Intended for --deep mode."""
    TIER_ORDER = ['0.01', '0.02-0.04', '0.05-0.09', '0.10-0.49', '0.50-0.99', '1.00+']
    by_tier: dict = defaultdict(lambda: {'pnl': 0.0, 'trades': 0, 'wins': 0})
    for deal in deals:
        if 'out' not in deal.get('entry', '').lower():
            continue
        profit = deal.get('profit', 0.0)
        if profit == 0.0:
            continue
        tier = _lot_tier(deal.get('volume', 0.0))
        by_tier[tier]['pnl']    += profit
        by_tier[tier]['trades'] += 1
        if profit > 0:
            by_tier[tier]['wins'] += 1

    return [
        {
            'lot_tier': tier,
            'pnl':      round(s['pnl'], 2),
            'trades':   s['trades'],
            'win_rate': round(s['wins'] / s['trades'] * 100, 1) if s['trades'] > 0 else 0.0,
        }
        for tier in TIER_ORDER
        if tier in by_tier
        for s in [by_tier[tier]]
    ]


# ── Strategy-aware analytics ──────────────────────────────────────────────────

def depth_histogram(deals: list[dict], profile: dict) -> dict:
    """
    Count how often each depth level was reached, using the profile's depth_re.
    Returns an empty dict when the profile has no depth_re (e.g. generic, scalper).
    For grid profiles, keys are L1 … L8+.
    """
    depth_re = profile.get('depth_re')
    if not depth_re:
        return {}

    hist: dict[str, int] = {'L1': 0, 'L2': 0, 'L3': 0, 'L4': 0,
                             'L5': 0, 'L6': 0, 'L7': 0, 'L8+': 0}
    for deal in deals:
        depth = _extract_depth(deal.get('comment', ''), depth_re)
        if depth:
            key = f'L{depth}' if depth <= 7 else 'L8+'
            hist[key] = hist.get(key, 0) + 1

    return hist


def grid_depth_histogram(deals: list[dict]) -> dict:
    """Backward-compatible alias: depth_histogram using the grid profile."""
    return depth_histogram(deals, PROFILES['grid'])


def cycle_stats(deals: list[dict], profile: Optional[dict] = None) -> dict:
    """
    Group deals into cycles using profile's cycle_group_by and cycle_gap_min.
    Returns overall win rate and win_rate_by_depth.

    Default profile (None) uses grid settings for backward compatibility.
    """
    _profile = profile or PROFILES['grid']
    group_by    = _profile.get('cycle_group_by', 'magic+direction')
    gap_minutes = _profile.get('cycle_gap_min', 60)
    depth_re    = _profile.get('depth_re')

    # active[key] = {'in_deals': [...], 'profit': float, 'max_depth': int, 'last_open': dt}
    active: dict[tuple, dict] = {}
    completed: list[dict] = []

    def _key(deal: dict) -> tuple:
        magic = deal.get('magic', '')
        if group_by == 'magic+direction':
            return (magic, deal.get('type', '').lower())
        return (magic,)

    def _flush(k: tuple) -> None:
        if k in active and active[k]['in_deals']:
            completed.append(active.pop(k))

    for deal in sorted(deals, key=lambda d: _parse_dt(d.get('time', '')) or datetime.min):
        entry = deal.get('entry', '').lower()
        dt    = _parse_dt(deal.get('time', ''))
        if not dt or not deal.get('magic', ''):
            continue

        key   = _key(deal)
        depth = _extract_depth(deal.get('comment', ''), depth_re)

        if 'in' in entry and 'out' not in entry:
            if key in active:
                gap = (dt - active[key]['last_open']).total_seconds() / 60
                if gap > gap_minutes:
                    _flush(key)
            if key not in active:
                active[key] = {'in_deals': [], 'profit': 0.0,
                               'max_depth': 0, 'last_open': dt}
            active[key]['in_deals'].append(deal)
            active[key]['last_open'] = dt
            active[key]['max_depth'] = max(active[key]['max_depth'], depth)

        elif 'out' in entry:
            profit = deal.get('profit', 0.0)
            if profit == 0.0:
                continue
            if key in active:
                active[key]['profit']   += profit
                active[key]['last_open'] = dt

    for k in list(active.keys()):
        _flush(k)

    if not completed:
        return {'total_cycles': 0, 'win_rate': 0.0, 'avg_profit': 0.0, 'win_rate_by_depth': {}}

    total        = len(completed)
    wins         = sum(1 for c in completed if c['profit'] > 0)
    total_profit = sum(c['profit'] for c in completed)

    by_depth: dict[str, dict] = defaultdict(lambda: {'wins': 0, 'total': 0})
    for c in completed:
        d = c['max_depth']
        label = (f'L{d}' if 0 < d <= 7 else 'L8+') if d > 0 else 'L?'
        by_depth[label]['total'] += 1
        if c['profit'] > 0:
            by_depth[label]['wins'] += 1

    return {
        'total_cycles': total,
        'win_rate':     round(wins / total * 100, 1),
        'avg_profit':   round(total_profit / total, 2),
        'win_rate_by_depth': {
            d: {'total': s['total'], 'win_rate': round(s['wins'] / s['total'] * 100, 1)}
            for d, s in sorted(by_depth.items())
        },
    }


def exit_reason_breakdown(deals: list[dict],
                          profile: Optional[dict] = None) -> dict:
    """
    Classify closed deals by exit reason and aggregate P/L.

    Default profile (None) uses grid keywords for backward compatibility.
    Generic profile returns only 'tp' / 'sl' (profit-sign classification).
    """
    _profile = profile or PROFILES['grid']
    counts: dict[str, int]   = defaultdict(int)
    pnl:    dict[str, float] = defaultdict(float)

    for deal in deals:
        if 'out' not in deal.get('entry', '').lower():
            continue
        profit = deal.get('profit', 0.0)
        if profit == 0.0:
            continue
        reason = _classify_exit(deal.get('comment', ''), profit, _profile)
        counts[reason] += 1
        pnl[reason]    += profit

    return {
        reason: {
            'count':     counts[reason],
            'total_pnl': round(pnl[reason], 2),
            'avg_pnl':   round(pnl[reason] / counts[reason], 2),
        }
        for reason in counts
    }


# ── Summary ───────────────────────────────────────────────────────────────────

def build_summary(metrics: dict, monthly: list[dict], dd_events: list[dict],
                  *,
                  streak:   Optional[dict] = None,
                  bias:     Optional[dict] = None,
                  exits:    Optional[dict] = None,
                  cycles:   Optional[dict] = None,
                  strategy: Optional[str]  = None) -> dict:
    green = sum(1 for m in monthly if m['green'])
    worst = min(monthly, key=lambda m: m['pnl'], default={})

    summary: dict = {
        'net_profit':      metrics.get('net_profit', 0),
        'profit_factor':   metrics.get('profit_factor', 0),
        'max_dd_pct':      metrics.get('max_dd_pct', 0),
        'sharpe_ratio':    metrics.get('sharpe_ratio', 0),
        'total_trades':    metrics.get('total_trades', 0),
        'recovery_factor': metrics.get('recovery_factor', 0),
        'green_months':    green,
        'total_months':    len(monthly),
        'worst_month':     worst.get('month', ''),
        'worst_month_pnl': worst.get('pnl', 0),
    }

    if strategy:
        summary['strategy'] = strategy

    if streak:
        summary['max_win_streak']     = streak.get('max_win_streak', 0)
        summary['max_loss_streak']    = streak.get('max_loss_streak', 0)
        summary['current_streak']     = streak.get('current_streak', 0)
        summary['current_streak_type']= streak.get('current_streak_type', '')

    if bias:
        for direction in ('buy', 'sell'):
            if direction in bias:
                summary[f'{direction}_win_rate']  = bias[direction].get('win_rate', 0)
                summary[f'{direction}_total_pnl'] = bias[direction].get('total_pnl', 0)

    if exits:
        dominant = max(exits, key=lambda k: exits[k]['count'], default='')
        summary['dominant_exit'] = dominant

    if cycles and cycles.get('total_cycles', 0):
        summary['cycle_win_rate'] = cycles.get('win_rate', 0)
        summary['total_cycles']   = cycles.get('total_cycles', 0)

    return summary


# ── Main ──────────────────────────────────────────────────────────────────────

def _make_parser(prog_suffix: str = '') -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog=f'mt5-analyze{prog_suffix}' if prog_suffix else 'analyze.py',
        description=f'MT5 deal analysis{(" — " + PROFILES[prog_suffix.lstrip("-")]["name"]) if prog_suffix else ""}',
    )
    p.add_argument('deals_csv',   help='Path to deals.csv')
    p.add_argument('--output-dir', default='.', help='Output directory')
    p.add_argument('--deep',   action='store_true', help='Add hourly_pnl and volume_profile')
    p.add_argument('--stdout', action='store_true', help='Print JSON to stdout')
    return p


def _run(deals_csv: str, output_dir: str, deep: bool, stdout: bool,
         strategy_name: str) -> None:
    profile = PROFILES.get(strategy_name, PROFILES['grid'])

    os.makedirs(output_dir, exist_ok=True)
    deals   = load_deals(deals_csv)
    metrics = load_metrics(os.path.join(output_dir, 'metrics.json'))

    if not deals:
        print("WARNING: No deals found in CSV", file=sys.stderr)

    # Generic analytics (always run, strategy-agnostic)
    monthly  = monthly_pnl(deals)
    dd_ev    = reconstruct_dd_events(deals, metrics, profile)
    top_loss = top_losses(deals)
    loss_seq = loss_sequences(deals)
    pairs    = position_pairs(deals)
    bias     = direction_bias(deals)
    streak   = streak_analysis(deals)
    sessions = session_breakdown(deals)
    wday     = weekday_pnl(deals)
    peak     = concurrent_peak(deals)

    # Strategy-aware analytics
    depth_hist = depth_histogram(deals, profile)
    cycles     = cycle_stats(deals, profile)
    exits      = exit_reason_breakdown(deals, profile)

    summary = build_summary(metrics, monthly, dd_ev,
                            streak=streak, bias=bias, exits=exits,
                            cycles=cycles, strategy=strategy_name)

    analysis: dict = {
        'strategy':              strategy_name,
        'summary':               summary,
        'monthly_pnl':           monthly,
        'dd_events':             dd_ev,
        'top_losses':            top_loss,
        'loss_sequences':        loss_seq,
        'position_pairs':        pairs,
        'direction_bias':        bias,
        'streak_analysis':       streak,
        'session_breakdown':     sessions,
        'weekday_pnl':           wday,
        'concurrent_peak':       peak,
        'depth_histogram':       depth_hist,
        'cycle_stats':           cycles,
        'exit_reason_breakdown': exits,
    }

    # Grid backward-compat alias
    if strategy_name == 'grid' and depth_hist:
        analysis['grid_depth_histogram'] = depth_hist

    if deep:
        analysis['hourly_pnl']    = hourly_pnl(deals)
        analysis['volume_profile'] = volume_profile(deals)

    if stdout:
        json.dump(analysis, sys.stdout, indent=2)
        print()
        return

    out_path = os.path.join(output_dir, 'analysis.json')
    with open(out_path, 'w') as f:
        json.dump(analysis, f, indent=2)

    print(f"Analysis complete [{PROFILES[strategy_name]['name']}]: {out_path}")
    print(f"  {summary.get('green_months', 0)}/{summary.get('total_months', 0)} green months")
    print(f"  {len(dd_ev)} DD events reconstructed")
    if depth_hist:
        max_layer = max(
            (k for k, v in depth_hist.items() if v > 0),
            key=lambda x: int(x[1:].replace('+', '9')),
            default='?'
        )
        print(f"  Grid depth: max {max_layer}")
    if cycles.get('total_cycles', 0):
        print(f"  {cycles['total_cycles']} cycles — {cycles['win_rate']}% win rate")
    if bias:
        for d in ('buy', 'sell'):
            if d in bias:
                b = bias[d]
                print(f"  {d.capitalize()}: {b['trades']} trades, "
                      f"{b['win_rate']}% win rate, {b['total_pnl']:+.2f}")


def main() -> None:
    """
    Entry point — auto-detects subcommand style vs legacy style.

      analyze.py grid deals.csv [options]   → grid strategy
      analyze.py deals.csv [options]        → grid (backward compat default)
    """
    strategy_name = 'grid'
    argv = sys.argv[1:]

    if argv and argv[0] in PROFILES:
        strategy_name = argv.pop(0)

    parser = _make_parser()
    args   = parser.parse_args(argv)
    _run(args.deals_csv, args.output_dir, args.deep, args.stdout, strategy_name)


# ── Named entry points for each strategy ─────────────────────────────────────

def main_generic() -> None:
    """Entry point: mt5-analyze (generic, strategy-agnostic)."""
    _entry('generic')

def main_grid() -> None:
    """Entry point: mt5-analyze-grid"""
    _entry('grid')

def main_scalper() -> None:
    """Entry point: mt5-analyze-scalper"""
    _entry('scalper')

def main_trend() -> None:
    """Entry point: mt5-analyze-trend"""
    _entry('trend')

def main_hedge() -> None:
    """Entry point: mt5-analyze-hedge"""
    _entry('hedge')

def _entry(strategy_name: str) -> None:
    parser = _make_parser(f'-{strategy_name}')
    args   = parser.parse_args()
    _run(args.deals_csv, args.output_dir, args.deep, args.stdout, strategy_name)


if __name__ == '__main__':
    main()
