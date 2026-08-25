#![no_main]

use libfuzzer_sys::fuzz_target;
use mt5_quant::models::Metrics;

// Fuzz target for the pure, I/O-free MT5 backtest report metrics parser.
//
// `Metrics::from_html` takes an arbitrary `&str` and extracts the numeric
// summary metrics from an MT5 HTML/XML report. It is panic-free on bad input:
// every numeric parse uses `unwrap_or`, regex patterns are compile-time
// constants (never panicking), and the function returns `Option<Metrics>`
// (None when no metrics are found). We return early on non-UTF-8 input and on
// the None case, so the fuzzer only ever exercises the parser itself.
fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        // Parser only accepts UTF-8 text; arbitrary byte sequences are
        // out of scope and must not cause a crash.
        return;
    };

    if let Some(metrics) = Metrics::from_html(text) {
        // Force the computed fields to be materialized so any logic error
        // inside the parser surfaces during fuzzing.
        let _ = (
            metrics.net_profit,
            metrics.profit_factor,
            metrics.max_dd_pct,
            metrics.sharpe_ratio,
            metrics.total_trades,
            metrics.recovery_factor,
            metrics.win_rate_pct,
            metrics.gross_profit,
            metrics.gross_loss,
        );
    }
});
