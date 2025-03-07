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
use std::collections::BTreeMap;
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
use tastytrade::api::quote_streaming::DxFeedSymbol;
use tastytrade::streaming::account_streaming::{AccountEvent, AccountMessage};
use tastytrade::utils::config::Config;
use tastytrade::{
    QuantityDirection, Symbol, TastyTrade,
    dxfeed::{self, Event, EventData},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// tastytrade username or email
    #[arg(short, long)]
    login: String,

    /// tastytrade password
    #[arg(short, long)]
    password: String,
}

#[derive(Debug)]
struct SimpleGreeks {
    theta: f64,
    delta: f64,
}

#[derive(Debug)]
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

    println!("Logging in...");
    let config = Config::from_env();
    let tasty = TastyTrade::login(&config)
        .await
        .context("Logging into tastytrade")?;

    println!("Downloading account info...");

    let account_streamer = tasty.create_account_streamer().await?;
    let mut positions = Vec::new();
    let mut balances = BTreeMap::new();
    for account in tasty.accounts().await.unwrap() {
        account_streamer.subscribe_to_account(&account).await;
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
    let quote_sub = quote_streamer.create_sub(dxfeed::DXF_ET_QUOTE | dxfeed::DXF_ET_GREEKS);
    quote_sub.add_symbols(&stream_syms);

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
                if let Ok(Event { sym, data }) = ev {
                    if let Some(record) = app.get_record(DxFeedSymbol(sym)) {
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
            }
            ev = account_streamer.get_event() => {
                if let Ok(AccountEvent::AccountMessage(msg)) = ev {
                    if let AccountMessage::AccountBalance(bal) = *msg {
                        app.balances.insert(bal.account_number.0, bal.cash_balance);
                    }
                }
            }
            maybe_event = keyboard_event_stream.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if let event::Event::Key(key) = event {
                            if key.kind == KeyEventKind::Press {
                                match key.code {
                                    KeyCode::Char('q') => break,
                                    KeyCode::Down => app.next(),
                                    KeyCode::Up => app.previous(),
                                    KeyCode::Char(' ') => app.toggle_group(),
                                    _ => {}
                                }
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
