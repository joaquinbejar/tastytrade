//! Start a backtest and poll it to completion.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin create_backtest
//! ```
//!
//! The full asynchronous lifecycle — create, poll, read logs — which is the
//! only way the shape gets exercised. It **simulates**: nothing routes, no
//! position changes, and no account is touched.

use std::time::Duration;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// How many times to poll before giving up. A backtest is long-running, and a
/// loop with no bound is a loop that hangs when the venue stops answering.
const MAX_POLLS: usize = 30;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }
    let tasty = TastyTrade::connect(&config).await?;

    let backtest = NewBacktest::new(
        "SPY",
        NaiveDate::from_ymd_opt(2024, 1, 1).expect("a real date"),
        NaiveDate::from_ymd_opt(2024, 3, 31).expect("a real date"),
        // A short put: `type` names the instrument and `side` names the
        // option side. They are different fields with different value sets.
        vec![BacktestLeg {
            leg_type: BacktestInstrument::EquityOption,
            direction: BacktestDirection::Short,
            quantity: Decimal::ONE,
            strike_selection: StrikeSelection::Delta,
            days_until_expiration: 45,
            side: Some(BacktestSide::Put),
            strike_relative_leg: None,
            delta: Some(Decimal::new(16, 2)),
            percentage_otm: None,
            current_price_offset: None,
            premium: None,
        }],
    )
    .with_exit_conditions(ExitConditions {
        take_profit_percentage: Some(50),
        at_days_to_expiration: Some(21),
        ..ExitConditions::default()
    });

    let created = match tasty.create_backtest(&backtest).await {
        Ok(created) => created,
        Err(error) => {
            info!("Backtesting did not accept the run: {error}");
            return Ok(());
        }
    };
    let Some(id) = created.id.clone() else {
        info!("The venue returned a backtest with no id, so it cannot be polled.");
        return Ok(());
    };
    info!("Started {id}");

    // The polling is the caller's, deliberately: how long to wait and what to
    // do meanwhile is not the library's decision.
    for attempt in 1..=MAX_POLLS {
        let run = tasty.backtest(&id).await?;
        info!(
            "  poll {attempt}: {} ({}%)",
            run.status.as_deref().unwrap_or("unstated"),
            run.progress
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string())
        );

        if run.is_finished() {
            info!(
                "Finished with {} trial(s) and {} snapshot(s)",
                run.trials.len(),
                run.snapshots.len()
            );
            for notice in &run.notices {
                info!("  notice: {notice}");
            }
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    match tasty.backtest_logs(&id).await {
        Ok(logs) => info!("Logs: {logs}"),
        Err(error) => info!("No logs: {error}"),
    }

    // Local refusals, none of which reach the network.
    let no_legs = NewBacktest::new(
        "SPY",
        NaiveDate::from_ymd_opt(2024, 1, 1).expect("a real date"),
        NaiveDate::from_ymd_opt(2024, 3, 31).expect("a real date"),
        vec![],
    );
    match tasty.create_backtest(&no_legs).await {
        Ok(_) => info!("a backtest with no legs was accepted, which is a bug"),
        Err(error) => info!(
            "no legs refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    Ok(())
}
