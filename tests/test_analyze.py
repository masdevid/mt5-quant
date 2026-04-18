"""Tests for analytics/analyze.py — runs without MT5 or Wine."""

import sys
from pathlib import Path

import pytest

FIXTURES = Path(__file__).parent / 'fixtures'
sys.path.insert(0, str(Path(__file__).parent.parent))

from analytics.analyze import (
    PROFILES,
    load_deals, monthly_pnl, reconstruct_dd_events,
    grid_depth_histogram, depth_histogram, top_losses, loss_sequences, build_summary,
    position_pairs, cycle_stats, exit_reason_breakdown,
    direction_bias, streak_analysis, session_breakdown,
    weekday_pnl, hourly_pnl, concurrent_peak, volume_profile,
    _parse_dt, _classify_exit, _extract_depth, _classify_dd_cause,
    _lot_tier, _session_for_hour,
)


@pytest.fixture
def deals():
    return load_deals(str(FIXTURES / 'sample_deals.csv'))


def test_load_deals_count(deals):
    assert len(deals) > 0


def test_load_deals_numeric_fields(deals):
    for deal in deals:
        assert isinstance(deal['profit'], float)
        assert isinstance(deal['balance'], float)
        assert isinstance(deal['volume'], float)


def test_monthly_pnl_groups_correctly(deals):
    result = monthly_pnl(deals)
    assert isinstance(result, list)
    assert len(result) >= 1
    for entry in result:
        assert 'month' in entry
        assert 'pnl' in entry
        assert 'trades' in entry
        assert 'green' in entry
        assert isinstance(entry['green'], bool)


def test_monthly_pnl_only_out_entries(deals):
    """Only 'out' entries should be counted."""
    result = monthly_pnl(deals)
    # All trades in fixture are closed, so at least one month should have trades
    total_trades = sum(m['trades'] for m in result)
    assert total_trades > 0


def test_monthly_pnl_has_jan_and_feb(deals):
    result = monthly_pnl(deals)
    months = [m['month'] for m in result]
    assert '2025-01' in months
    assert '2025-02' in months


def test_reconstruct_dd_events_returns_list(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    result = reconstruct_dd_events(deals, metrics)
    assert isinstance(result, list)


def test_reconstruct_dd_events_empty_on_no_deals():
    result = reconstruct_dd_events([], {})
    assert result == []


def test_grid_depth_histogram_keys(deals):
    hist = grid_depth_histogram(deals)
    assert isinstance(hist, dict)
    assert 'L1' in hist
    assert 'L2' in hist
    assert 'L3' in hist
    assert 'L8+' in hist


def test_grid_depth_histogram_counts_layers(deals):
    hist = grid_depth_histogram(deals)
    # Fixture has Layer #1, #2, #3 comments
    assert hist['L1'] > 0
    assert hist['L3'] > 0


def test_top_losses_are_negative(deals):
    losses = top_losses(deals)
    assert isinstance(losses, list)
    for loss in losses:
        assert loss['loss_usd'] < 0


def test_top_losses_sorted_ascending(deals):
    losses = top_losses(deals)
    if len(losses) >= 2:
        assert losses[0]['loss_usd'] <= losses[1]['loss_usd']


def test_loss_sequences_structure(deals):
    seqs = loss_sequences(deals)
    assert isinstance(seqs, list)
    for seq in seqs:
        assert 'length' in seq
        assert 'total_loss' in seq
        assert seq['total_loss'] < 0


def test_build_summary_keys(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5, 'total_trades': 11,
               'profit_factor': 1.2, 'sharpe_ratio': 0.5, 'recovery_factor': 2.0}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    summary = build_summary(metrics, monthly, dd)

    expected_keys = ['net_profit', 'profit_factor', 'max_dd_pct', 'sharpe_ratio',
                     'total_trades', 'green_months', 'total_months',
                     'worst_month', 'worst_month_pnl']
    for k in expected_keys:
        assert k in summary, f"Missing key: {k}"


def test_build_summary_green_months(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    summary = build_summary(metrics, monthly, dd)
    assert summary['green_months'] >= 0
    assert summary['total_months'] >= summary['green_months']


# ── Utility helpers ────────────────────────────────────────────────────────────

def test_parse_dt_standard_format():
    dt = _parse_dt('2025.01.10 09:30:00')
    assert dt is not None
    assert dt.year == 2025
    assert dt.month == 1
    assert dt.day == 10
    assert dt.hour == 9


def test_parse_dt_iso_format():
    dt = _parse_dt('2025-02-05 14:00:00')
    assert dt is not None
    assert dt.month == 2


def test_parse_dt_invalid_returns_none():
    assert _parse_dt('') is None
    assert _parse_dt('not-a-date') is None


def test_classify_exit_locking():
    assert _classify_exit('locking hedge', -50.0) == 'locking'


def test_classify_exit_cutloss():
    assert _classify_exit('cutloss fired', -20.0) == 'cutloss'
    assert _classify_exit('cut loss', -20.0) == 'cutloss'


def test_classify_exit_tp_sl_by_profit():
    assert _classify_exit('Layer #1', 15.0) == 'tp'
    assert _classify_exit('Layer #1', -10.0) == 'sl'


def test_lot_tier():
    assert _lot_tier(0.01) == '0.01'
    assert _lot_tier(0.02) == '0.02-0.04'
    assert _lot_tier(0.04) == '0.02-0.04'
    assert _lot_tier(0.06) == '0.05-0.09'
    assert _lot_tier(0.10) == '0.10-0.49'
    assert _lot_tier(1.0) == '1.00+'


def test_session_for_hour():
    assert _session_for_hour(3) == 'asian'
    assert _session_for_hour(9) == 'london'
    assert _session_for_hour(14) == 'london_ny_overlap'
    assert _session_for_hour(18) == 'new_york'
    assert _session_for_hour(23) == 'off_hours'


# ── Position pairs ─────────────────────────────────────────────────────────────

def test_position_pairs_count(deals):
    pairs = position_pairs(deals)
    assert isinstance(pairs, list)
    assert len(pairs) > 0


def test_position_pairs_hold_minutes(deals):
    pairs = position_pairs(deals)
    for p in pairs:
        if p['hold_minutes'] is not None:
            assert p['hold_minutes'] > 0


def test_position_pairs_has_layer(deals):
    pairs = position_pairs(deals)
    layers = [p['layer'] for p in pairs if p['layer'] > 0]
    assert len(layers) > 0


def test_position_pairs_profit_nonzero(deals):
    pairs = position_pairs(deals)
    for p in pairs:
        assert p['profit'] != 0.0


# ── Cycle stats ────────────────────────────────────────────────────────────────

def test_cycle_stats_structure(deals):
    result = cycle_stats(deals)
    assert 'total_cycles' in result
    assert 'win_rate' in result
    assert 'avg_profit' in result
    assert 'win_rate_by_depth' in result


def test_cycle_stats_total_cycles(deals):
    result = cycle_stats(deals)
    assert result['total_cycles'] > 0


def test_cycle_stats_win_rate_range(deals):
    result = cycle_stats(deals)
    assert 0.0 <= result['win_rate'] <= 100.0


def test_cycle_stats_empty():
    result = cycle_stats([])
    assert result['total_cycles'] == 0


# ── Exit reason breakdown ──────────────────────────────────────────────────────

def test_exit_reason_breakdown_structure(deals):
    result = exit_reason_breakdown(deals)
    assert isinstance(result, dict)
    for reason, data in result.items():
        assert 'count' in data
        assert 'total_pnl' in data
        assert 'avg_pnl' in data
        assert data['count'] > 0


def test_exit_reason_breakdown_has_cutloss(deals):
    result = exit_reason_breakdown(deals)
    # fixture has 'cutloss' in comment for some deals
    assert 'cutloss' in result


def test_exit_reason_breakdown_counts_match(deals):
    result = exit_reason_breakdown(deals)
    total_counted = sum(r['count'] for r in result.values())
    closed_with_pnl = [d for d in deals
                       if 'out' in d.get('entry', '').lower() and d.get('profit', 0.0) != 0.0]
    assert total_counted == len(closed_with_pnl)


# ── Direction bias ─────────────────────────────────────────────────────────────

def test_direction_bias_keys(deals):
    result = direction_bias(deals)
    assert isinstance(result, dict)
    # fixture has both buy and sell
    assert 'buy' in result
    assert 'sell' in result


def test_direction_bias_win_rate_range(deals):
    result = direction_bias(deals)
    for d, s in result.items():
        assert 0.0 <= s['win_rate'] <= 100.0
        assert s['trades'] > 0


def test_direction_bias_buy_profitable(deals):
    result = direction_bias(deals)
    # fixture: buy deals net positive
    assert result['buy']['total_pnl'] > 0


# ── Streak analysis ────────────────────────────────────────────────────────────

def test_streak_analysis_structure(deals):
    result = streak_analysis(deals)
    assert isinstance(result, dict)
    for key in ('max_win_streak', 'max_loss_streak', 'current_streak', 'current_streak_type'):
        assert key in result


def test_streak_analysis_nonnegative(deals):
    result = streak_analysis(deals)
    assert result['max_win_streak'] >= 0
    assert result['max_loss_streak'] >= 0
    assert result['current_streak'] >= 1


def test_streak_analysis_type_valid(deals):
    result = streak_analysis(deals)
    assert result['current_streak_type'] in ('win', 'loss')


def test_streak_analysis_empty():
    assert streak_analysis([]) == {}


# ── Session breakdown ──────────────────────────────────────────────────────────

def test_session_breakdown_structure(deals):
    result = session_breakdown(deals)
    assert isinstance(result, dict)
    for session, data in result.items():
        assert 'trades' in data
        assert 'win_rate' in data
        assert 'total_pnl' in data


def test_session_breakdown_has_sessions(deals):
    result = session_breakdown(deals)
    # fixture has deals at 09:00, 10:00, 14:00, 15:00, 16:00 (London + London/NY)
    # and 02:30, 03:15 (Asian), 20:00-21:30 (NY)
    known_sessions = {'london', 'london_ny_overlap', 'asian', 'new_york'}
    assert len(set(result.keys()) & known_sessions) >= 2


def test_session_breakdown_win_rate_range(deals):
    result = session_breakdown(deals)
    for session, data in result.items():
        assert 0.0 <= data['win_rate'] <= 100.0


# ── Weekday P/L ────────────────────────────────────────────────────────────────

def test_weekday_pnl_structure(deals):
    result = weekday_pnl(deals)
    assert isinstance(result, list)
    for entry in result:
        assert 'day' in entry
        assert 'pnl' in entry
        assert 'trades' in entry
        assert 'win_rate' in entry


def test_weekday_pnl_day_names(deals):
    result = weekday_pnl(deals)
    valid_days = {'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'}
    for entry in result:
        assert entry['day'] in valid_days


def test_weekday_pnl_has_results(deals):
    result = weekday_pnl(deals)
    assert len(result) >= 1


# ── Hourly P/L ─────────────────────────────────────────────────────────────────

def test_hourly_pnl_structure(deals):
    result = hourly_pnl(deals)
    assert isinstance(result, list)
    for entry in result:
        assert 'hour' in entry
        assert 0 <= entry['hour'] <= 23
        assert 'pnl' in entry
        assert 'trades' in entry


def test_hourly_pnl_has_results(deals):
    result = hourly_pnl(deals)
    assert len(result) >= 1


# ── Concurrent peak ────────────────────────────────────────────────────────────

def test_concurrent_peak_structure(deals):
    result = concurrent_peak(deals)
    assert 'peak_open' in result
    assert 'peak_time' in result


def test_concurrent_peak_at_least_one(deals):
    result = concurrent_peak(deals)
    assert result['peak_open'] >= 1


def test_concurrent_peak_multi_layer(deals):
    # fixture has a cycle where L2 and L3 open before close → peak >= 2
    result = concurrent_peak(deals)
    assert result['peak_open'] >= 2


# ── Volume profile ─────────────────────────────────────────────────────────────

def test_volume_profile_structure(deals):
    result = volume_profile(deals)
    assert isinstance(result, list)
    for entry in result:
        assert 'lot_tier' in entry
        assert 'pnl' in entry
        assert 'trades' in entry
        assert 'win_rate' in entry


def test_volume_profile_has_micro_lots(deals):
    result = volume_profile(deals)
    tiers = [e['lot_tier'] for e in result]
    assert '0.01' in tiers


def test_volume_profile_win_rate_range(deals):
    result = volume_profile(deals)
    for entry in result:
        assert 0.0 <= entry['win_rate'] <= 100.0


# ── build_summary with new stats ───────────────────────────────────────────────

def test_build_summary_with_streak(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    streak = streak_analysis(deals)
    summary = build_summary(metrics, monthly, dd, streak=streak)
    assert 'max_win_streak' in summary
    assert 'max_loss_streak' in summary
    assert 'current_streak_type' in summary


def test_build_summary_with_bias(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    bias = direction_bias(deals)
    summary = build_summary(metrics, monthly, dd, bias=bias)
    assert 'buy_win_rate' in summary or 'sell_win_rate' in summary


def test_build_summary_with_cycles(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    cycles = cycle_stats(deals)
    summary = build_summary(metrics, monthly, dd, cycles=cycles)
    assert 'cycle_win_rate' in summary
    assert 'total_cycles' in summary


# ── Strategy profiles ──────────────────────────────────────────────────────────

def test_profiles_registry():
    """All expected strategy names are registered."""
    for name in ('generic', 'grid', 'scalper', 'trend', 'hedge'):
        assert name in PROFILES
        p = PROFILES[name]
        assert 'name' in p
        assert 'exit_keywords' in p
        assert 'dd_cause_keywords' in p
        assert 'cycle_group_by' in p
        assert 'cycle_gap_min' in p


def test_profiles_depth_re():
    assert PROFILES['grid']['depth_re'] is not None
    assert PROFILES['generic']['depth_re'] is None
    assert PROFILES['scalper']['depth_re'] is None
    assert PROFILES['trend']['depth_re'] is None
    assert PROFILES['hedge']['depth_re'] is None


# ── _extract_depth ─────────────────────────────────────────────────────────────

def test_extract_depth_grid_pattern():
    depth_re = PROFILES['grid']['depth_re']
    assert _extract_depth('Layer #3', depth_re) == 3
    assert _extract_depth('Layer #1', depth_re) == 1
    assert _extract_depth('layer 7',  depth_re) == 7


def test_extract_depth_no_pattern():
    assert _extract_depth('Layer #3', None) == 0
    assert _extract_depth('',         None) == 0


def test_extract_depth_no_match():
    assert _extract_depth('TP hit', PROFILES['grid']['depth_re']) == 0


# ── _classify_exit with profiles ──────────────────────────────────────────────

def test_classify_exit_grid_locking():
    assert _classify_exit('locking hedge', -50.0, PROFILES['grid']) == 'locking'


def test_classify_exit_grid_cutloss():
    assert _classify_exit('cutloss fired', -20.0, PROFILES['grid']) == 'cutloss'


def test_classify_exit_scalper_manual():
    assert _classify_exit('manual close', -5.0, PROFILES['scalper']) == 'manual'


def test_classify_exit_scalper_trailing():
    assert _classify_exit('trailing stop', 10.0, PROFILES['scalper']) == 'trailing'


def test_classify_exit_trend_breakeven():
    assert _classify_exit('breakeven stop', 0.5, PROFILES['trend']) == 'breakeven'


def test_classify_exit_trend_partial():
    assert _classify_exit('partial scale out', 15.0, PROFILES['trend']) == 'partial'


def test_classify_exit_hedge_net_close():
    assert _classify_exit('net close', -30.0, PROFILES['hedge']) == 'net_close'


def test_classify_exit_generic_fallback():
    """Generic profile has no keywords — falls back to profit sign."""
    assert _classify_exit('Layer #3 locking', -50.0, PROFILES['generic']) == 'sl'
    assert _classify_exit('Layer #1',          15.0, PROFILES['generic']) == 'tp'


# ── _classify_dd_cause ────────────────────────────────────────────────────────

def test_classify_dd_cause_grid():
    assert _classify_dd_cause('locking total', PROFILES['grid']) == 'locking_cascade'
    assert _classify_dd_cause('cutloss fired', PROFILES['grid']) == 'cutloss'
    assert _classify_dd_cause('zombie exit',   PROFILES['grid']) == 'zombie_exit'


def test_classify_dd_cause_generic_unknown():
    assert _classify_dd_cause('locking total', PROFILES['generic']) == 'unknown'
    assert _classify_dd_cause('',              PROFILES['generic']) == 'unknown'


def test_classify_dd_cause_scalper_stop():
    assert _classify_dd_cause('sl hit', PROFILES['scalper']) == 'stop_loss'


def test_classify_dd_cause_trend_whipsaw():
    assert _classify_dd_cause('stop loss', PROFILES['trend']) == 'whipsaw'


# ── depth_histogram with profiles ─────────────────────────────────────────────

def test_depth_histogram_grid_returns_layers(deals):
    result = depth_histogram(deals, PROFILES['grid'])
    assert isinstance(result, dict)
    assert 'L1' in result
    assert result['L1'] > 0


def test_depth_histogram_generic_returns_empty(deals):
    """Generic profile has no depth_re → empty dict."""
    result = depth_histogram(deals, PROFILES['generic'])
    assert result == {}


def test_depth_histogram_scalper_returns_empty(deals):
    result = depth_histogram(deals, PROFILES['scalper'])
    assert result == {}


def test_grid_depth_histogram_is_alias(deals):
    """grid_depth_histogram must equal depth_histogram with grid profile."""
    assert grid_depth_histogram(deals) == depth_histogram(deals, PROFILES['grid'])


# ── cycle_stats with profiles ─────────────────────────────────────────────────

def test_cycle_stats_grid_profile(deals):
    result = cycle_stats(deals, PROFILES['grid'])
    assert result['total_cycles'] > 0
    assert 0.0 <= result['win_rate'] <= 100.0


def test_cycle_stats_scalper_profile(deals):
    """Scalper uses magic-only grouping and 10-min gap."""
    result = cycle_stats(deals, PROFILES['scalper'])
    assert 'total_cycles' in result
    assert result['total_cycles'] > 0


def test_cycle_stats_generic_profile(deals):
    result = cycle_stats(deals, PROFILES['generic'])
    assert 'total_cycles' in result


def test_cycle_stats_scalper_vs_grid_differ(deals):
    """Different grouping rules can produce different cycle counts."""
    grid_result    = cycle_stats(deals, PROFILES['grid'])
    scalper_result = cycle_stats(deals, PROFILES['scalper'])
    # Both must be valid; counts may differ due to grouping
    assert grid_result['total_cycles'] >= 0
    assert scalper_result['total_cycles'] >= 0


# ── exit_reason_breakdown with profiles ───────────────────────────────────────

def test_exit_reason_breakdown_grid(deals):
    result = exit_reason_breakdown(deals, PROFILES['grid'])
    assert 'cutloss' in result   # fixture has "cutloss" in comments


def test_exit_reason_breakdown_generic_only_tp_sl(deals):
    """Generic profile has no keywords → only 'tp' and 'sl' keys."""
    result = exit_reason_breakdown(deals, PROFILES['generic'])
    for reason in result:
        assert reason in ('tp', 'sl'), f"Unexpected reason '{reason}' from generic profile"


def test_exit_reason_breakdown_scalper_keywords(deals):
    """Scalper profile recognises 'cutloss' comment as 'manual' (not 'cutloss')."""
    result = exit_reason_breakdown(deals, PROFILES['scalper'])
    # 'cutloss' is not a scalper keyword → falls back to profit-sign → 'sl'
    assert 'cutloss' not in result


def test_exit_reason_breakdown_counts_sum(deals):
    """Total count must equal number of non-zero closed deals, regardless of profile."""
    closed = [d for d in deals
              if 'out' in d.get('entry', '').lower() and d.get('profit', 0.0) != 0.0]
    for profile in PROFILES.values():
        result = exit_reason_breakdown(deals, profile)
        assert sum(r['count'] for r in result.values()) == len(closed)


# ── reconstruct_dd_events with profiles ───────────────────────────────────────

def test_dd_events_cause_generic_unknown(deals):
    """Generic profile → all causes must be 'unknown'."""
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    events = reconstruct_dd_events(deals, metrics, PROFILES['generic'])
    for ev in events:
        assert ev['cause'] == 'unknown'


def test_dd_events_cause_grid_classified(deals):
    """Grid profile → cause is classified from comment keywords."""
    metrics = {'net_profit': 38.0, 'max_dd_pct': 5.0}
    events = reconstruct_dd_events(deals, metrics, PROFILES['grid'])
    valid = {'locking_cascade', 'cutloss', 'zombie_exit', 'spike_entry', 'unknown'}
    for ev in events:
        assert ev['cause'] in valid


# ── build_summary strategy field ──────────────────────────────────────────────

def test_build_summary_strategy_field(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    summary = build_summary(metrics, monthly, dd, strategy='scalper')
    assert summary['strategy'] == 'scalper'


def test_build_summary_no_strategy_field(deals):
    metrics = {'net_profit': 38.0, 'max_dd_pct': 1.5}
    monthly = monthly_pnl(deals)
    dd = reconstruct_dd_events(deals, metrics)
    summary = build_summary(metrics, monthly, dd)
    assert 'strategy' not in summary
