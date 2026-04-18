"""Tests for analytics/extract.py — runs without MT5 or Wine."""

import json
import os
import sys
import tempfile
from pathlib import Path

import pytest

FIXTURES = Path(__file__).parent / 'fixtures'
sys.path.insert(0, str(Path(__file__).parent.parent))

from analytics.extract import (
    detect_format, parse_html, parse_xml, write_outputs,
    _parse_metrics_html, _parse_deals_html,
)


def test_detect_format_html():
    assert detect_format(str(FIXTURES / 'sample_report.htm')) == 'html'


def test_detect_format_xml():
    assert detect_format(str(FIXTURES / 'sample_report.htm.xml')) == 'xml'


# HTML parsing is tested via internal functions to avoid the UTF-16 decode dance
# (read_text tries UTF-16 first, which silently garbles plain UTF-8/ASCII files).
# The fixture is used for format-detection only.

HTML_TEXT = """<html><body>
<table>
<tr><td>Net profit</td><td>1234.56</td></tr>
<tr><td>Profit factor</td><td>1.25</td></tr>
<tr><td>Maximal drawdown</td><td>500.00 (5.00%)</td></tr>
<tr><td>Sharpe Ratio</td><td>0.75</td></tr>
<tr><td>Total trades</td><td>150</td></tr>
<tr><td>Recovery factor</td><td>2.50</td></tr>
<tr><td>Profit trades (% of total)</td><td>90 (60.00%)</td></tr>
<tr><td>Gross profit</td><td>2000.00</td></tr>
<tr><td>Gross loss</td><td>-765.44</td></tr>
</table>
<table>
<tr><td>Deal Time</td><td>Type</td><td>Direction</td><td>Volume</td><td>Price</td><td>S/L</td><td>T/P</td><td>Profit</td><td>Balance</td><td>Comment</td><td>Order</td><td>Magic</td><td>Entry</td></tr>
<tr><td>2025.01.10 09:30:00</td><td>buy</td><td>out</td><td>0.01</td><td>1915.00</td><td>0</td><td>0</td><td>15.00</td><td>10015.00</td><td>Layer #1</td><td>1001</td><td>12345</td><td>out</td></tr>
<tr><td>2025.02.05 14:00:00</td><td>sell</td><td>out</td><td>0.01</td><td>1945.00</td><td>0</td><td>0</td><td>-15.00</td><td>10020.00</td><td>Layer #1</td><td>1005</td><td>12345</td><td>out</td></tr>
</table>
</body></html>"""


@pytest.fixture
def html_report_path(tmp_path):
    """Write HTML fixture as UTF-16 LE with BOM so read_text() decodes it correctly."""
    p = tmp_path / 'report.htm'
    p.write_bytes(b'\xff\xfe' + HTML_TEXT.encode('utf-16-le'))
    return str(p)


def test_parse_html_returns_metrics(html_report_path):
    metrics, _ = parse_html(html_report_path)
    assert isinstance(metrics, dict)
    assert 'net_profit' in metrics
    assert metrics['net_profit'] == pytest.approx(1234.56)
    assert metrics['total_trades'] == 150


def test_parse_html_returns_deals(html_report_path):
    _, deals = parse_html(html_report_path)
    assert isinstance(deals, list)
    assert len(deals) >= 1
    deal = deals[0]
    assert 'profit' in deal
    assert 'balance' in deal


def test_parse_metrics_html_directly():
    """Test HTML metric extraction without encoding layer."""
    metrics = _parse_metrics_html(HTML_TEXT)
    assert metrics['net_profit'] == pytest.approx(1234.56)
    assert metrics['profit_factor'] == pytest.approx(1.25)
    assert metrics['max_dd_pct'] == pytest.approx(5.00)
    assert metrics['total_trades'] == 150


def test_parse_deals_html_directly():
    """Test HTML deal extraction without encoding layer."""
    deals = _parse_deals_html(HTML_TEXT)
    assert len(deals) == 2
    assert float(deals[0]['profit']) == pytest.approx(15.00)
    assert float(deals[1]['profit']) == pytest.approx(-15.00)


def test_parse_xml_returns_metrics():
    metrics, deals = parse_xml(str(FIXTURES / 'sample_report.htm.xml'))
    assert isinstance(metrics, dict)
    assert 'net_profit' in metrics
    assert metrics['net_profit'] == pytest.approx(1234.56)
    assert metrics['total_trades'] == 150


def test_parse_xml_returns_deals():
    metrics, deals = parse_xml(str(FIXTURES / 'sample_report.htm.xml'))
    assert isinstance(deals, list)
    assert len(deals) >= 1
    deal = deals[0]
    assert deal.get('profit') is not None


def test_write_outputs_creates_files():
    metrics = {'net_profit': 100.0, 'total_trades': 5}
    deals = [
        {'time': '2025.01.10', 'type': 'buy', 'direction': 'out', 'volume': '0.01',
         'price': '1900', 'sl': '0', 'tp': '0', 'profit': '10.00',
         'balance': '10010', 'comment': '', 'order': '1', 'magic': '1', 'entry': 'out'},
    ]
    with tempfile.TemporaryDirectory() as tmp:
        paths = write_outputs(metrics, deals, tmp)
        assert Path(paths['metrics']).exists()
        assert Path(paths['deals_csv']).exists()
        assert Path(paths['deals_json']).exists()
        # Verify metrics.json content
        with open(paths['metrics']) as f:
            saved = json.load(f)
        assert saved['net_profit'] == 100.0


def test_parse_html_skips_balance_rows():
    """Rows with type='balance' should be filtered out."""
    html = """
    <table>
    <tr><td>Deal Time</td><td>Type</td><td>Direction</td><td>Volume</td><td>Price</td><td>S/L</td><td>T/P</td><td>Profit</td><td>Balance</td><td>Comment</td><td>Order</td><td>Magic</td><td>Entry</td></tr>
    <tr><td>2025.01.10 09:30:00</td><td>balance</td><td></td><td>0</td><td>0</td><td>0</td><td>0</td><td>0</td><td>10000</td><td></td><td>0</td><td>0</td><td></td></tr>
    <tr><td>2025.01.10 10:00:00</td><td>buy</td><td>out</td><td>0.01</td><td>1910</td><td>0</td><td>0</td><td>5.00</td><td>10005</td><td>Layer #1</td><td>1</td><td>1</td><td>out</td></tr>
    </table>
    """
    import tempfile, os
    with tempfile.NamedTemporaryFile(mode='w', suffix='.htm', delete=False) as f:
        f.write(html)
        path = f.name
    try:
        _, deals = parse_html(path)
        types = [d.get('type', '').lower() for d in deals]
        assert 'balance' not in types
    finally:
        os.unlink(path)
