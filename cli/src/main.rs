/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 5/3/25
******************************************************************************/

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

use rust_decimal::{
    Decimal,
    prelude::{FromPrimitive, Zero},
};
use serde::Serialize;
use tastytrade::api::quote_streaming::DxFeedSymbol;
use tastytrade::streaming::account_streaming::{AccountEvent, AccountMessage};
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::{
    QuantityDirection, Symbol, TastyTrade,
    dxfeed::{self, Event, EventData},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Read credentials from this JSON configuration file instead of the
    /// environment.
    ///
    /// Neither OAuth credential is a flag. A client secret or refresh token
    /// given on the command line is visible to every process on the machine
    /// and is kept in the shell history, and both are worth as much as a
    /// password. `--login` is gone with the API it named: tastytrade retired
    /// username and password authentication on 2026-02-11.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

/// Applies the command line on top of the environment.
///
/// Both OAuth credentials come from the environment or from `--config`; there
/// is no flag that carries one.
fn resolve_config(config: TastyTradeConfig, args: &Args) -> Result<TastyTradeConfig> {
    let config = match &args.config {
        Some(path) => TastyTradeConfig::from_file(path)
            .with_context(|| format!("could not read {}", path.display()))?,
        None => config,
    };

    // Checked here so the message can name the missing piece. Connecting
    // reports this too, but by then the operator has already been told which
    // environment, which reads as though the credential was fine.
    if !config.has_valid_credentials() {
        anyhow::bail!(
            "no OAuth credentials: set TASTYTRADE_CLIENT_SECRET and \
             TASTYTRADE_REFRESH_TOKEN, or pass --config with a file that \
             provides both. Create them under Manage > My Profile > API on \
             my.tastytrade.com. There is no flag for either: a secret given on \
             the command line is visible to every process on the machine and is \
             kept in the shell history"
        );
    }

    Ok(config)
}

#[derive(DebugPretty, DisplaySimple, Serialize)]
struct SimpleGreeks {
    theta: f64,
    delta: f64,
}

#[derive(DebugPretty, DisplaySimple, Serialize)]
struct PriceRecord {
    symbol: Symbol,
    open: Decimal,
    current: Decimal,
    amount: Decimal,
    multiplier: Decimal,
    direction: QuantityDirection,
    greeks: SimpleGreeks,
}

#[derive(Default)]
struct UnderlyingGroup {
    open: bool,
    pub records: BTreeMap<DxFeedSymbol, PriceRecord>,
}

struct App {
    state: TableState,
    groups: BTreeMap<Symbol, UnderlyingGroup>,
    num_lines: usize,
    balances: BTreeMap<String, Decimal>,
}

impl App {
    fn new(
        records: BTreeMap<Symbol, UnderlyingGroup>,
        balances: BTreeMap<String, Decimal>,
    ) -> Self {
        let mut this = Self {
            state: TableState::default(),
            groups: records,
            num_lines: 0,
            balances,
        };

        this.update_num_lines();
        this
    }

    pub fn update_num_lines(&mut self) {
        self.num_lines = self.groups.values().fold(self.groups.len(), |acc, group| {
            acc + if group.open { group.records.len() } else { 0 }
        });
    }

    pub fn toggle_group(&mut self) {
        let selected = match self.state.selected() {
            Some(s) => s,
            None => return,
        };
        let mut i = 0;
        for group in self.groups.values_mut() {
            if i == selected {
                group.open = !group.open;
                break;
            }
            i += 1;
            if group.open {
                i += group.records.len()
            }
        }
        self.update_num_lines();
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.num_lines - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.num_lines - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn get_record(&mut self, symbol: DxFeedSymbol) -> Option<&mut PriceRecord> {
        for positions in self.groups.values_mut() {
            for (pos_symbol, pos) in positions.records.iter_mut() {
                if *pos_symbol == symbol {
                    return Some(pos);
                }
            }
        }
        None
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = resolve_config(TastyTradeConfig::from_env(), &args)?;

    // Which deployment, never who: the environment is what an operator needs
    // to know before watching an account, and certification reuses production
    // account numbering, so "which one am I looking at" is a real question.
    println!("Logging in to {}...", config.environment());
    let tasty = TastyTrade::connect(&config)
        .await
        .context("Logging into tastytrade")?;

    println!("Downloading account info...");

    let account_streamer = tasty.create_account_streamer().await?;
    let mut positions = Vec::new();
    let mut balances = BTreeMap::new();
    for account in tasty.accounts().await.unwrap() {
        account_streamer
            .subscribe_to_account(&account)
            .await
            .context("subscribing to account updates")?;
        positions.extend(account.positions().await.unwrap());
        balances.insert(account.number().0, account.balance().await?.cash_balance);
    }

    println!("Downloading symbols...");
    let sym_futures = positions
        .iter()
        .map(|pos| tasty.get_streamer_symbol(&pos.instrument_type, &pos.symbol));

    let stream_syms = futures::future::join_all(sym_futures).await;
    let stream_syms: Result<Vec<_>, _> = stream_syms.into_iter().collect();
    let stream_syms = stream_syms?;

    println!("Setting up records...");
    let mut records: BTreeMap<Symbol, UnderlyingGroup> = BTreeMap::new();
    for (pos, stream_sym) in positions.iter().zip(stream_syms.iter()) {
        let record = PriceRecord {
            symbol: pos.symbol.clone(),
            open: pos.average_open_price.round_dp(2),
            current: pos.close_price.round_dp(2),
            amount: pos.quantity,
            multiplier: pos.multiplier,
            direction: pos.quantity_direction,
            greeks: SimpleGreeks {
                theta: 0.0,
                delta: 0.0,
            },
        };
        records
            .entry(pos.underlying_symbol.clone())
            .or_default()
            .records
            .insert(stream_sym.clone(), record);
    }

    print!("Setting up quote streaming...");
    let mut quote_streamer = tasty.create_quote_streamer().await?;
    let mut quote_sub = quote_streamer.create_sub(dxfeed::DXF_ET_QUOTE | dxfeed::DXF_ET_GREEKS);
    quote_sub
        .add_symbols(&stream_syms)
        .await
        .context("subscribing to quote symbols")?;

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(records, balances);
    let mut keyboard_event_stream = EventStream::new();

    loop {
        tokio::select! {
            ev = quote_sub.get_event() => {
                if let Ok(Event { sym, data }) = ev
                    && let Some(record) = app.get_record(DxFeedSymbol(sym)) {
                        match data {
                            EventData::Quote(quote) => {
                                record.current = Decimal::from_f64((quote.bid_price + quote.ask_price) / 2.0).unwrap_or_default();
                            }
                            EventData::Greeks(greeks) => {
                                record.greeks = SimpleGreeks {
                                    theta: greeks.theta,
                                    delta: greeks.delta,
                                }
                            }
                            _ => {}
                        }
                    }
            }
            ev = account_streamer.get_event() => {
                if let Ok(AccountEvent::AccountMessage(msg)) = ev
                    && let AccountMessage::AccountBalance(bal) = *msg {
                        app.balances.insert(bal.account_number.0, bal.cash_balance);
                    }
            }
            maybe_event = keyboard_event_stream.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if let event::Event::Key(key) = event
                            && key.kind == KeyEventKind::Press {
                                match key.code {
                                    KeyCode::Char('q') => break,
                                    KeyCode::Down => app.next(),
                                    KeyCode::Up => app.previous(),
                                    KeyCode::Char(' ') => app.toggle_group(),
                                    _ => {}
                                }
                            }
                    }
                    Some(Err(e)) => println!("Error: {:?}\r", e),
                    None => break,
                }
            }
        }

        terminal.draw(|f| ui(f, &mut app))?;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Percentage(100)].as_ref())
        .margin(2)
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::Blue);
    let header_cells = [
        "PORT %",
        "SYMBOL",
        "CURRENT",
        "AMOUNT",
        "TRADE PRICE",
        "PROFIT",
        "THETA",
        "DELTA",
        "NET LIQ",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));
    let header = Row::new(header_cells).style(normal_style).height(1);

    let mut total = app.groups.iter().fold(Decimal::zero(), |acc, (_, group)| {
        acc + group.records.iter().fold(Decimal::zero(), |acc, (_, rec)| {
            acc + (rec.current
                * rec.amount
                * rec.multiplier
                * if let QuantityDirection::Short = rec.direction {
                    Decimal::from(-1)
                } else {
                    Decimal::from(1)
                })
            .round_dp(2)
        })
    });

    let mut rows: Vec<Row> = app
        .groups
        .iter()
        .flat_map(|(underlying_symbol, records)| {
            let mut rows = vec![vec![]];
            let mut profit_sum = Decimal::zero();
            let mut net_liq_sum = Decimal::zero();
            for rec in records.records.values() {
                let to_net = |value: Decimal| -> Decimal {
                    (value
                        * rec.amount
                        * rec.multiplier
                        * if let QuantityDirection::Short = rec.direction {
                            Decimal::from(-1)
                        } else {
                            Decimal::from(1)
                        })
                    .round_dp(2)
                };
                let profit = to_net(rec.current - rec.open);
                profit_sum += profit;

                let net_liq = to_net(rec.current);
                net_liq_sum += net_liq;

                if !records.open {
                    continue;
                }
                let theta = to_net(Decimal::from_f64(rec.greeks.theta).unwrap());
                let delta = to_net(Decimal::from_f64(rec.greeks.delta).unwrap());

                let name = if rec.symbol == *underlying_symbol {
                    "SHARES".to_owned()
                } else {
                    rec.symbol.0.clone()
                };
                let cells = vec![
                    ((net_liq * Decimal::from_u64(100).unwrap()) / total)
                        .round_dp(2)
                        .to_string()
                        + "%",
                    format!(" {}", name),
                    rec.current.round_dp(2).to_string(),
                    (rec.amount
                        * if let QuantityDirection::Short = rec.direction {
                            Decimal::from(-1)
                        } else {
                            Decimal::from(1)
                        })
                    .round_dp(5)
                    .to_string(),
                    rec.open.to_string(),
                    profit.to_string(),
                    theta.to_string(),
                    delta.to_string(),
                    net_liq.to_string(),
                ];
                rows.push(cells)
            }

            let group_header = rows.get_mut(0).unwrap();
            group_header.extend(vec![
                ((net_liq_sum * Decimal::from_u64(100).unwrap()) / total)
                    .round_dp(2)
                    .to_string()
                    + "%",
                underlying_symbol.0.clone(),
                "".to_owned(),
                "".to_owned(),
                "".to_owned(),
                profit_sum.round_dp(2).to_string(),
                "".to_owned(),
                "".to_owned(),
                net_liq_sum.round_dp(2).to_string(),
            ]);

            rows.into_iter().map(Row::new)
        })
        .collect();

    rows.push(Row::new(vec![""]));
    rows.push(Row::new(vec!["CASH"]));
    for (account, balance) in &app.balances {
        rows.push(Row::new(vec![
            " ".to_owned() + account,
            balance.to_string(),
        ]));
        total += balance;
    }
    rows.push(Row::new(vec![""]));
    rows.push(Row::new(vec!["TOTAL".to_owned(), total.to_string()]));

    let t = Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(selected_style)
        .highlight_symbol(">> ")
        .widths(&[
            Constraint::Length(8),
            Constraint::Length(25),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ]);

    f.render_stateful_widget(t, rects[0], &mut app.state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tastytrade::prelude::{ClientSecret, RefreshToken};

    const SECRET: &str = "SENTINEL-client-secret-3Qv7";
    const REFRESH: &str = "SENTINEL-refresh-token-8Hb2";

    fn config() -> TastyTradeConfig {
        TastyTradeConfig {
            client_secret: ClientSecret::new(SECRET),
            refresh_token: RefreshToken::new(REFRESH),
            client_id: String::new(),
            redirect_uri: String::new(),
            use_demo: true,
            log_level: "INFO".to_string(),
            base_url: "https://api.cert.tastyworks.com".to_string(),
            websocket_url: "wss://streamer.cert.tastyworks.com".to_string(),
        }
    }

    /// Without a flag the environment decides, which is how every existing
    /// invocation works.
    #[test]
    fn without_a_flag_the_environment_decides() {
        let resolved = resolve_config(config(), &Args { config: None }).expect("resolves");

        assert_eq!(resolved.client_secret.expose_secret(), SECRET);
        assert_eq!(resolved.refresh_token.expose_secret(), REFRESH);
    }

    /// The message has to name the variables, and has to say that the flags it
    /// would be natural to reach for do not exist, so the reader stops looking
    /// for them.
    #[test]
    fn missing_credentials_name_the_variables_and_not_a_flag() {
        let missing = TastyTradeConfig {
            refresh_token: RefreshToken::new(""),
            ..config()
        };

        let error = resolve_config(missing, &Args { config: None })
            .expect_err("there is nothing to authenticate with");
        let message = format!("{error}");

        assert!(message.contains("TASTYTRADE_CLIENT_SECRET"), "{message}");
        assert!(message.contains("TASTYTRADE_REFRESH_TOKEN"), "{message}");
        assert!(
            message.contains("no flag for either"),
            "the message must say the flags are absent on purpose: {message}"
        );
    }

    /// A shell accident is not a credential, and the message must not suggest
    /// the venue refused anything.
    #[test]
    fn whitespace_is_not_a_credential() {
        let blank = TastyTradeConfig {
            client_secret: ClientSecret::new("   "),
            ..config()
        };

        let error =
            resolve_config(blank, &Args { config: None }).expect_err("whitespace is not a secret");
        assert!(
            format!("{error}").contains("no OAuth credentials"),
            "{error}"
        );
    }

    /// No credential may appear in an error a user will paste into a bug
    /// report.
    #[test]
    fn no_error_repeats_a_credential() {
        let error = resolve_config(
            TastyTradeConfig {
                refresh_token: RefreshToken::new(""),
                ..config()
            },
            &Args { config: None },
        )
        .expect_err("no refresh token");

        assert!(!format!("{error}").contains("SENTINEL"), "{error}");
    }

    /// A missing configuration file is the operator's mistake, and the message
    /// names the path rather than reporting a bare io error.
    #[test]
    fn a_missing_config_file_names_the_path() {
        let error = resolve_config(
            config(),
            &Args {
                config: Some(PathBuf::from("/nonexistent/tastytrade-cli.json")),
            },
        )
        .expect_err("the file is not there");

        assert!(
            format!("{error}").contains("/nonexistent/tastytrade-cli.json"),
            "{error}"
        );
    }
}
