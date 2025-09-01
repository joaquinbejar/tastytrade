use super::order::Symbol;
use crate::api::quote_streaming::DxFeedSymbol;
use chrono::{DateTime, Utc};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fmt::Display;

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
pub struct CompactOptionChainResponse {
    pub data: CompactOptionChainData,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
pub struct CompactOptionChainData {
    pub items: Vec<CompactOptionChain>,
}

/// Represents a compact option chain with simplified strike information.
///
/// This structure provides a more streamlined representation of an option chain
/// compared to the full `NestedOptionChain`, focusing on essential information
/// for quick access and reduced memory usage.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CompactOptionChain {
    /// The symbol of the underlying asset (e.g., "AAPL").
    pub underlying_symbol: Symbol,

    /// The root symbol of the option chain (e.g., "AAPL").
    pub root_symbol: Symbol,

    /// The type of the option chain (e.g., "equity", "future").
    pub option_chain_type: String,

    /// The settlement type of the option chain.
    pub settlement_type: Option<String>,

    /// The number of shares represented by each option contract.
    pub shares_per_contract: u64,

    /// The expiration type of the option chain.
    pub expiration_type: Option<String>,

    /// Compact representation of symbols as a string.
    pub symbols: Option<Vec<String>>,

    /// Compact representation of streamer symbols as a string.
    pub streamer_symbols: Option<Vec<String>>,
}

/// Represents the different types of financial instruments.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InstrumentType {
    /// Represents an equity instrument.
    Equity,
    /// Represents an equity option instrument.
    #[serde(rename = "Equity Option")]
    EquityOption,
    /// Represents an equity offering instrument.
    #[serde(rename = "Equity Offering")]
    EquityOffering,
    /// Represents a future instrument.
    Future,
    /// Represents a future option instrument.
    #[serde(rename = "Future Option")]
    FutureOption,
    /// Represents a cryptocurrency instrument.
    Cryptocurrency,
    /// Represents a bond instrument.
    Bond,
    /// Represents a fixed income security instrument.
    #[serde(rename = "Fixed Income Security")]
    FixedIncomeSecurity,
    /// Represents a liquidity pool instrument.
    #[serde(rename = "Liquidity Pool")]
    LiquidityPool,
    /// Represents a warrant instrument.
    Warrant,
}

impl Display for InstrumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstrumentType::Equity => write!(f, "Equity"),
            InstrumentType::EquityOption => write!(f, "Equity Option"),
            InstrumentType::EquityOffering => write!(f, "Equity Offering"),
            InstrumentType::Future => write!(f, "Future"),
            InstrumentType::FutureOption => write!(f, "Future Option"),
            InstrumentType::Cryptocurrency => write!(f, "Cryptocurrency"),
            InstrumentType::Bond => write!(f, "Bond"),
            InstrumentType::FixedIncomeSecurity => write!(f, "Fixed Income Security"),
            InstrumentType::LiquidityPool => write!(f, "Liquidity Pool"),
            InstrumentType::Warrant => write!(f, "Warrant"),
        }
    }
}

/// Represents equity instrument information.
///
/// This struct holds the symbol and the streamer symbol for an equity instrument.
/// It uses kebab-case for serialization and deserialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityInstrumentInfo {
    /// The symbol of the equity instrument.
    pub symbol: Symbol,
    /// The streamer symbol of the equity instrument.
    pub streamer_symbol: DxFeedSymbol,
}

/// Represents a tick size, which is the minimum price movement of a financial instrument.
///
/// This struct is deserialized from a JSON response using `serde`.
/// The fields are renamed to kebab-case during deserialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TickSize {
    /// The value of the tick size.
    pub value: String,
    /// An optional threshold associated with the tick size.
    pub threshold: Option<String>,
}

/// Represents an equity instrument.
///
/// This struct is deserialized from a JSON response using `serde`.
/// The fields are renamed to kebab-case during deserialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityInstrument {
    /// The unique identifier of the equity instrument.
    pub id: u64,
    /// The symbol of the equity instrument.
    pub symbol: Symbol,
    /// The type of the instrument. Always `InstrumentType::Equity` for this struct.
    pub instrument_type: InstrumentType,
    /// The CUSIP (Committee on Uniform Securities Identification Procedures) number of the equity instrument.
    pub cusip: Option<String>,
    /// A short description of the equity instrument.
    pub short_description: String,
    /// Indicates whether the instrument is an index.
    pub is_index: bool,
    /// The market where the equity instrument is listed.
    pub listed_market: String,
    /// A detailed description of the equity instrument.
    pub description: String,
    /// The lendability of the equity instrument.
    pub lendability: Option<String>,
    /// The borrow rate of the equity instrument.
    pub borrow_rate: Option<String>,
    /// The market time instrument collection.
    pub market_time_instrument_collection: String,
    /// Indicates whether the instrument is closing only.
    pub is_closing_only: bool,
    /// Indicates whether the instrument's options are closing only.
    pub is_options_closing_only: bool,
    /// Indicates whether the instrument is active.
    pub active: bool,
    /// Indicates whether the instrument is eligible for fractional quantity trading.
    #[serde(default)]
    pub is_fractional_quantity_eligible: bool,
    /// Indicates whether the instrument is illiquid.
    pub is_illiquid: bool,
    /// Indicates whether the instrument is an ETF (Exchange Traded Fund).
    pub is_etf: bool,
    /// Indicates whether the instrument bypasses manual review.
    pub bypass_manual_review: bool,
    /// Indicates whether the instrument is a fraud risk.
    pub is_fraud_risk: bool,
    /// The symbol used by the DxFeed data stream.
    pub streamer_symbol: DxFeedSymbol,
    /// A vector of tick sizes for the instrument.
    pub tick_sizes: Option<Vec<TickSize>>,
    /// A vector of tick sizes for the instrument's options.
    pub option_tick_sizes: Option<Vec<TickSize>>,
}

/// Represents a strike price for options trading.
///
/// This struct holds information about a specific strike price, including its monetary value
/// and the associated call and put option symbols.  It uses symbols specifically designed for
/// interaction with the DxFeed data stream.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Strike {
    /// The strike price itself, represented as a Decimal for precision.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,

    /// The symbol for the call option at this strike price.
    pub call: Symbol,

    /// The DxFeed-specific symbol for the call option, used for streaming data.
    pub call_streamer_symbol: DxFeedSymbol,

    /// The symbol for the put option at this strike price.
    pub put: Symbol,

    /// The DxFeed-specific symbol for the put option, used for streaming data.
    pub put_streamer_symbol: DxFeedSymbol,
}

/// Represents an expiration date for a set of options.
///
/// This struct holds information about a specific expiration date for a particular
/// underlying asset. It includes details such as the type of expiration, the date
/// itself, the number of days until expiration, the settlement type, and a
/// vector of `Strike` structs representing the available strike prices for
/// this expiration date.  The data structure uses kebab-case for its fields
/// to match the format of incoming data.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Expiration {
    /// The type of expiration (e.g., "weekly", "monthly").
    pub expiration_type: String,

    /// The date of expiration in string format (e.g., "2024-12-20").
    pub expiration_date: String,

    /// The number of days remaining until expiration.
    pub days_to_expiration: u64,

    /// The settlement type for the options (e.g., "cash", "physical").
    pub settlement_type: String,

    /// A vector of `Strike` structs, each representing a different strike price
    /// available for this expiration date.
    pub strikes: Vec<Strike>,
}

/// Represents a nested option chain for a specific underlying symbol.
///
/// This structure encapsulates the details of an option chain,
/// including information about the underlying and root symbols,
/// the type of option chain, the number of shares per contract,
/// and a collection of expiration dates along with their associated
/// strike prices.  The data structure uses kebab-case for its fields
/// to match the format of incoming data.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NestedOptionChain {
    /// The symbol of the underlying asset (e.g., "AAPL").
    pub underlying_symbol: Symbol,

    /// The root symbol of the option chain (e.g., "AAPL").
    pub root_symbol: Symbol,

    /// The type of the option chain (e.g., "equity", "future").
    pub option_chain_type: String,

    /// The number of shares represented by each option contract.
    pub shares_per_contract: u64,

    /// A vector of `Expiration` structs, each representing a different
    /// expiration date for the option chain.
    pub expirations: Vec<Expiration>,
}

/// Represents a futures nested option chain response.
///
/// This structure matches the FuturesNestedOptionChainSerializer from the API,
/// containing both futures information and option chains data.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FuturesNestedOptionChain {
    /// Array of futures contracts information.
    pub futures: Vec<FuturesInfo>,
    
    /// Array of option chains data for the futures contracts.
    pub option_chains: Vec<FuturesOptionChains>,
}

/// Represents futures contract information.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FuturesInfo {
    /// The symbol of the futures contract.
    pub symbol: String,
    
    /// The root symbol of the futures contract.
    pub root_symbol: String,
    
    /// The expiration date of the futures contract.
    pub expiration_date: String,
    
    /// Days to expiration of the futures contract.
    pub days_to_expiration: i32,
    
    /// Whether this is an active month contract.
    pub active_month: bool,
    
    /// Whether this is the next active month contract.
    pub next_active_month: bool,
    
    /// When the futures contract stops trading.
    pub stops_trading_at: String,
    
    /// When the futures contract expires.
    pub expires_at: String,
}

/// Represents the option chains section of futures nested option chain.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FuturesOptionChains {
    /// The underlying symbol for the options.
    pub underlying_symbol: String,
    
    /// The root symbol for the options.
    pub root_symbol: String,
    
    /// The exercise style of the options.
    pub exercise_style: String,
    
    /// The expirations data for the option chain.
    pub expirations: Vec<FuturesExpiration>,
}

/// Represents an expiration in a futures option chain.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FuturesExpiration {
    /// The underlying symbol.
    pub underlying_symbol: String,
    
    /// The root symbol.
    pub root_symbol: String,
    
    /// The option root symbol.
    pub option_root_symbol: String,
    
    /// The option contract symbol.
    pub option_contract_symbol: String,
    
    /// The asset identifier.
    pub asset: String,
    
    /// The expiration date.
    pub expiration_date: String,
    
    /// Days to expiration.
    pub days_to_expiration: i32,
    
    /// The expiration type.
    pub expiration_type: String,
    
    /// The settlement type.
    pub settlement_type: String,
    
    /// The notional value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub notional_value: Decimal,
    
    /// The display factor.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub display_factor: Decimal,
    
    /// The strike factor.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_factor: Decimal,
    
    /// When trading stops.
    pub stops_trading_at: String,
    
    /// When the option expires.
    pub expires_at: String,
    
    /// Tick sizes information.
    pub tick_sizes: Vec<FuturesTickSize>,
    
    /// Strike prices and option symbols.
    pub strikes: Vec<FuturesStrike>,
}

/// Represents tick size information for futures options.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FuturesTickSize {
    /// The threshold value (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<String>,
    
    /// The tick size value.
    pub value: String,
}

/// Represents a strike price and associated option symbols for futures.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FuturesStrike {
    /// The strike price.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    
    /// The call option symbol.
    pub call: String,
    
    /// The call option streamer symbol (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_streamer_symbol: Option<String>,
    
    /// The put option symbol.
    pub put: String,
    
    /// The put option streamer symbol (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_streamer_symbol: Option<String>,
}

/// Represents an equity option.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityOption {
    /// The symbol of the equity option.
    pub symbol: Symbol,
    /// The type of the instrument.  This should always be `InstrumentType::EquityOption`.
    pub instrument_type: InstrumentType,
    /// Whether the option is active.
    pub active: bool,
    /// The strike price of the option.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    /// The root symbol of the option.
    pub root_symbol: Symbol,
    /// The symbol of the underlying asset.
    pub underlying_symbol: Symbol,
    /// The expiration date of the option, formatted as a string.
    pub expiration_date: String,
    /// The exercise style of the option (e.g., "American").
    pub exercise_style: String,
    /// The number of shares per contract.
    pub shares_per_contract: u64,
    /// The type of the option (e.g., "CALL", "PUT").
    pub option_type: String,
    /// The type of the option chain.
    pub option_chain_type: String,
    /// The type of expiration.
    pub expiration_type: String,
    /// The settlement type.
    pub settlement_type: String,
    /// The date and time when the option stops trading, formatted as a string.
    pub stops_trading_at: String,
    /// The market time instrument collection.
    pub market_time_instrument_collection: String,
    /// The number of days to expiration (can be negative for expired options).
    pub days_to_expiration: i64,
    /// The date and time when the option expires, formatted as a string.
    pub expires_at: String,
    /// Whether the option is closing only.
    pub is_closing_only: bool,
    /// The streamer symbol for the future option.
    pub streamer_symbol: Option<DxFeedSymbol>,
}

/// Represents a future contract.
///
/// This struct is deserialized from a JSON response using `serde`.
/// The fields are renamed to kebab-case during deserialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Future {
    /// The symbol of the future contract.
    pub symbol: Symbol,
    /// The product code of the future.
    pub product_code: String,
    /// The contract size of the future.
    pub contract_size: String,
    /// The tick size of the future.
    pub tick_size: String,
    /// The notional multiplier of the future.
    pub notional_multiplier: String,
    /// The main fraction of the future.
    pub main_fraction: String,
    /// The sub-fraction of the future.
    pub sub_fraction: String,
    /// The display factor of the future.
    pub display_factor: String,
    /// The last trade date of the future.
    pub last_trade_date: String,
    /// The expiration date of the future.
    pub expiration_date: String,
    /// The closing only date of the future.
    pub closing_only_date: Option<String>,
    /// Whether the future is active.
    pub active: bool,
    /// Whether the future is in the active month.
    pub active_month: bool,
    /// Whether the future is in the next active month.
    pub next_active_month: bool,
    /// Whether the future is closing only.
    pub is_closing_only: bool,
    /// The time at which the future stops trading.
    pub stops_trading_at: String,
    /// The time at which the future expires.
    pub expires_at: String,
    /// The product group of the future.
    pub product_group: String,
    /// The exchange on which the future is traded.
    pub exchange: String,
    /// The roll target symbol of the future.
    pub roll_target_symbol: Option<Symbol>,
    /// The streamer exchange code of the future.
    pub streamer_exchange_code: String,
    /// The streamer symbol of the future.
    pub streamer_symbol: DxFeedSymbol,
    /// Whether the future is a back month first calendar symbol.
    pub back_month_first_calendar_symbol: bool,
    /// Whether the future is tradeable.
    pub is_tradeable: bool,
    /// The future product.
    pub future_product: FutureProduct,
    /// The tick sizes of the future.
    #[serde(default)]
    pub tick_sizes: Vec<TickSize>,
    /// The option tick sizes of the future.
    #[serde(default)]
    pub option_tick_sizes: Vec<TickSize>,
    /// The spread tick sizes of the future.
    pub spread_tick_sizes: Option<Vec<HashMap<String, String>>>,
}

/// Represents a future product.
///
/// This struct holds information about a future product, including its symbol, codes,
/// description, exchange details, product type, listed and active months, and various
/// other characteristics.  It utilizes the `kebab-case` naming convention for serialization
/// and deserialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureProduct {
    /// The root symbol of the future product.
    pub root_symbol: Symbol,
    /// The code of the future product.
    pub code: String,
    /// The description of the future product.
    pub description: String,
    /// The clearing code of the future product.
    pub clearing_code: String,
    /// The clearing exchange code of the future product.
    pub clearing_exchange_code: String,
    /// The clearport code of the future product.
    pub clearport_code: Option<String>,
    /// The legacy code of the future product.
    pub legacy_code: Option<String>,
    /// The exchange where the future product is traded.
    pub exchange: String,
    /// The legacy exchange code of the future product.
    pub legacy_exchange_code: Option<String>,
    /// The type of the future product.
    pub product_type: String,
    /// A list of strings representing the listed months for the future product.
    pub listed_months: Vec<String>,
    /// A list of strings representing the active months for the future product.
    pub active_months: Vec<String>,
    /// The notional multiplier for the future product.
    pub notional_multiplier: String,
    /// The tick size for the future product.
    pub tick_size: String,
    /// The display factor for the future product.
    pub display_factor: String,
    /// The streamer exchange code for the future product.
    pub streamer_exchange_code: String,
    /// A boolean indicating whether the future product has a small notional value.
    pub small_notional: bool,
    /// A boolean indicating whether the back month is the first calendar symbol.
    pub back_month_first_calendar_symbol: bool,
    /// A boolean indicating whether the future product has a first notice.
    pub first_notice: bool,
    /// A boolean indicating whether the future product is cash settled.
    pub cash_settled: bool,
    /// The security group of the future product.
    pub security_group: Option<String>,
    /// The market sector of the future product.
    pub market_sector: String,
    /// Information about the roll of the future product.
    pub roll: FutureRoll,
}

/// Represents a future roll.
///
/// This struct holds information about a future roll, including its name,
/// active count, whether it's cash-settled, the business days offset, and
/// if it's the first notice.
///
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureRoll {
    /// The name of the future roll.
    pub name: String,
    /// The active count of the future roll.
    pub active_count: u32,
    /// Whether the future roll is cash-settled.
    pub cash_settled: bool,
    /// The business days offset for the future roll.
    pub business_days_offset: u32,
    /// Whether the future roll is the first notice.
    pub first_notice: bool,
}

/// Represents a future option.
///
/// This struct encapsulates the details of a future option, including its symbol,
/// underlying symbol, product code, expiration date, strike price, exchange
/// information, and various other characteristics.  It utilizes the
/// `serde` crate for serialization and deserialization, with a `kebab-case`
/// naming convention.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureOption {
    /// The symbol of the future option.
    pub symbol: Symbol,
    /// The symbol of the underlying asset.
    pub underlying_symbol: Symbol,
    /// The product code of the future option.
    pub product_code: String,
    /// The expiration date of the future option.
    pub expiration_date: String,
    /// The root symbol of the future option.
    pub root_symbol: Symbol,
    /// The option root symbol.
    pub option_root_symbol: String,
    /// The strike price of the future option.  Uses arbitrary precision
    /// deserialization via the `rust_decimal` crate.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    /// The exchange where the future option is traded.
    pub exchange: String,
    /// The exchange symbol of the future option.
    pub exchange_symbol: String,
    /// The streamer symbol for the future option.
    pub streamer_symbol: Option<DxFeedSymbol>,
    /// The type of the option (e.g., "call", "put").
    pub option_type: String,
    /// The exercise style of the option (e.g., "american", "european").
    pub exercise_style: String,
    /// Indicates whether the option is vanilla.
    pub is_vanilla: bool,
    /// Indicates whether the option is the primary deliverable.
    pub is_primary_deliverable: bool,
    /// The future price ratio.
    pub future_price_ratio: String,
    /// The multiplier for the future option.
    pub multiplier: String,
    /// The underlying count for the future option.
    pub underlying_count: String,
    /// Indicates whether the future option is confirmed.
    pub is_confirmed: bool,
    /// The notional value of the future option.
    pub notional_value: String,
    /// The display factor for the future option.
    pub display_factor: String,
    /// The security exchange for the future option.
    pub security_exchange: String,
    /// The SX ID of the future option.
    pub sx_id: String,
    /// The settlement type of the future option.
    pub settlement_type: String,
    /// The strike factor for the future option.
    pub strike_factor: String,
    /// The maturity date of the future option.
    pub maturity_date: String,
    /// Indicates whether the future option is exercisable weekly.
    pub is_exercisable_weekly: bool,
    /// The last trade time of the future option.
    pub last_trade_time: String,
    /// The number of days to expiration.
    pub days_to_expiration: i32,
    /// Indicates if the future option is closing only.
    pub is_closing_only: bool,
    /// Indicates whether the future option is active.
    pub active: bool,
    /// The date and time when the future option stops trading.
    pub stops_trading_at: String,
    /// The date and time when the future option expires.
    pub expires_at: String,
    /// Information about the future option product.
    pub future_option_product: FutureOptionProduct,
}

/// Represents a future option product.
///
/// This struct holds information about a future option product, including details
/// such as its root symbol, settlement type, various codes, exchange, product
/// type, expiration type, and other relevant attributes.  It's designed to be
/// serialized and deserialized using the `serde` library, with field names
/// converted to kebab-case.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureOptionProduct {
    /// The root symbol of the future option.
    pub root_symbol: String,
    /// Indicates whether the future option is cash-settled.
    pub cash_settled: bool,
    /// The code of the future option.
    pub code: String,
    /// The legacy code of the future option.
    pub legacy_code: Option<String>,
    /// The ClearPort code of the future option.
    pub clearport_code: Option<String>,
    /// The clearing code of the future option.
    pub clearing_code: String,
    /// The clearing exchange code of the future option.
    pub clearing_exchange_code: String,
    /// The clearing price multiplier of the future option.
    pub clearing_price_multiplier: String,
    /// The display factor of the future option.
    pub display_factor: String,
    /// The exchange where the future option is traded.
    pub exchange: String,
    /// The type of the product (e.g., "future option").
    pub product_type: String,
    /// The type of expiration for the future option.
    pub expiration_type: String,
    /// The number of days for settlement delay.
    pub settlement_delay_days: u32,
    /// Indicates whether the future option is a rollover.
    pub is_rollover: bool,
    /// The market sector of the future option.
    pub market_sector: String,
    /// Whether the future option product is supported.
    pub supported: Option<bool>,
    /// Trading cutoff times for futures.
    pub futures_trading_cutoff_times: Option<Vec<serde_json::Value>>,
}

/// Represents a cryptocurrency instrument.
///
/// This struct holds information about a cryptocurrency instrument, including its ID, symbol,
/// instrument type, description, trading restrictions, activity status, tick size,
/// streamer symbol, and destination venue symbols.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Cryptocurrency {
    /// The unique identifier for the cryptocurrency.
    pub id: u64,
    /// The symbol of the cryptocurrency.
    pub symbol: Symbol,
    /// The type of instrument, which should always be `InstrumentType::Cryptocurrency`.
    pub instrument_type: InstrumentType,
    /// A short description of the cryptocurrency.
    pub short_description: String,
    /// A more detailed description of the cryptocurrency.
    pub description: String,
    /// Indicates whether trading is restricted to closing only.
    pub is_closing_only: bool,
    /// Indicates whether the cryptocurrency is currently active for trading.
    pub active: bool,
    /// The tick size for the cryptocurrency, represented as a string.
    pub tick_size: String,
    /// The symbol used by the data streamer (DxFeed).
    pub streamer_symbol: DxFeedSymbol,
    /// A vector of destination venue symbols for the cryptocurrency.
    pub destination_venue_symbols: Vec<DestinationVenueSymbol>,
}

/// Represents a destination venue symbol.
///
/// This struct holds information about a specific symbol traded on a particular
/// destination venue. It includes details such as the symbol's ID, the symbol
/// itself, the destination venue name, precision for quantity and price, and
/// whether the symbol is routable.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DestinationVenueSymbol {
    /// The unique identifier for the symbol.
    pub id: u64,
    /// The symbol itself, represented as a `Symbol` struct.
    pub symbol: Symbol,
    /// The name of the destination venue where the symbol is traded.
    pub destination_venue: String,
    /// The maximum precision allowed for quantity values.
    pub max_quantity_precision: Option<u32>,
    /// The maximum precision allowed for price values.
    pub max_price_precision: Option<u32>,
    /// Indicates whether the symbol is routable.
    pub routable: bool,
}

/// Represents a Warrant instrument.
///
/// Warrants are derivative securities that give the holder the right, but not the obligation,
/// to buy or sell an underlying asset at a certain price before expiration.  
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Warrant {
    /// The symbol of the warrant.
    pub symbol: Symbol,
    /// The type of instrument, which for a warrant should always be `InstrumentType::Warrant`.
    pub instrument_type: InstrumentType,
    /// The market where the warrant is listed.
    pub listed_market: String,
    /// A description of the warrant.
    pub description: String,
    /// Indicates whether the warrant can only be closed (i.e., sold if held long, or bought back if held short) and not opened (i.e., bought or sold short).
    pub is_closing_only: bool,
    /// Indicates whether the warrant is currently active.
    pub active: bool,
}

/// Represents the decimal precision for a given instrument.
///
/// This struct is used to define the precision for quantity values, as well as the minimum increment
/// allowed for that quantity.  The `value` field represents the number of decimal places allowed for
/// a quantity, while the `minimum_increment_precision` field specifies the number of decimal places
/// required for the minimum increment.  For instance, a `value` of 2 and a `minimum_increment_precision`
/// of 2 would allow quantities like 1.23, and the minimum increment would also need to be expressed
/// with two decimal places (e.g., 0.01).
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuantityDecimalPrecision {
    /// The type of instrument.  Examples include `Equity`, `EquityOption`, `Future`, etc.
    pub instrument_type: InstrumentType,
    /// The symbol of the instrument (optional).  This field can be `None` for certain instrument types.
    pub symbol: Option<Symbol>,
    /// The number of decimal places allowed for quantity values.  This effectively sets the precision
    /// for quantity representation.
    pub value: u32,
    /// The number of decimal places required for the minimum increment value.  This ensures that
    /// the minimum increment is represented with the correct level of precision.
    pub minimum_increment_precision: u32,
}

/// Structure to hold symbol information from TastyTrade
#[derive(Clone, Serialize, Deserialize, DebugPretty, DisplaySimple)]
pub struct SymbolEntry {
    /// The trading symbol identifier
    pub symbol: String,
    /// The Epic identifier used by the exchange
    pub epic: String,
    /// Human-readable name of the instrument
    pub name: String,
    /// Instrument type classification
    pub instrument_type: InstrumentType,
    /// The exchange where this instrument is traded
    pub exchange: String,
    /// Expiration date and time for the instrument
    pub expiry: DateTime<Utc>,
    /// Timestamp of the last update to this record
    pub last_update: DateTime<Utc>,
}

impl PartialEq for SymbolEntry {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol && self.epic == other.epic
    }
}

impl Eq for SymbolEntry {}

impl std::hash::Hash for SymbolEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
        self.epic.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_equity_option_deserialization() {
        let json = r#"{
            "active": true,
            "strike-price": "150.00",
            "root-symbol": "AAPL",
            "underlying-symbol": "AAPL",
            "expiration-date": "2024-01-19",
            "exercise-style": "American",
            "shares-per-contract": 100,
            "option-type": "C",
            "option-chain-type": "Standard",
            "symbol": "AAPL  240119C00150000",
            "exchange": "CBOE",
            "exchange-symbol": "AAPL   240119C00150000",
            "is-closing-only": false,
            "days-to-expiration": 30,
            "expires-at": "2024-01-19T21:00:00.000+00:00",
            "is-confirmed": true,
            "stops-trading-at": "2024-01-19T21:00:00.000+00:00",
            "streamer-symbol": "AAPL_011924C150"
        }"#;

        let option: EquityOption = serde_json::from_str(json).unwrap();
        assert_eq!(option.symbol.0, "AAPL  240119C00150000");
        assert_eq!(option.underlying_symbol.0, "AAPL");
        assert_eq!(option.strike_price, Decimal::from_str("150.00").unwrap());
        assert_eq!(option.option_type, "C");
        assert_eq!(option.shares_per_contract, 100);
    }

    #[test]
    fn test_futures_nested_option_chain_deserialization() {
        // Test with a simplified version of the real JSON structure
        let json = r#"{
            "futures": [
                {
                    "symbol": "/ESU5",
                    "root-symbol": "/ES",
                    "expiration-date": "2025-09-19",
                    "days-to-expiration": 18,
                    "active-month": true,
                    "next-active-month": false,
                    "stops-trading-at": "2025-09-19T13:30:00.000+00:00",
                    "expires-at": "2025-09-19T13:30:00.000+00:00"
                }
            ],
            "option-chains": [
                {
                    "underlying-symbol": "/ES",
                    "root-symbol": "/ES",
                    "exercise-style": "American",
                    "expirations": [
                        {
                            "underlying-symbol": "/ESZ5",
                            "root-symbol": "/ES",
                            "option-root-symbol": "ES",
                            "option-contract-symbol": "ESZ5",
                            "asset": "ES",
                            "expiration-date": "2025-12-19",
                            "days-to-expiration": 109,
                            "expiration-type": "Regular",
                            "settlement-type": "AM",
                            "notional-value": "0.5",
                            "display-factor": "0.01",
                            "strike-factor": "1.0",
                            "stops-trading-at": "2025-12-19T14:30:00.000+00:00",
                            "expires-at": "2025-12-19T14:30:00.000+00:00",
                            "tick-sizes": [
                                {
                                    "threshold": "10.0",
                                    "value": "0.05"
                                },
                                {
                                    "value": "0.25"
                                }
                            ],
                            "strikes": [
                                {
                                    "strike-price": "800.0",
                                    "call": "./ESZ5 ESZ5  251219C800",
                                    "call-streamer-symbol": "./ESZ25C800:XCME",
                                    "put": "./ESZ5 ESZ5  251219P800",
                                    "put-streamer-symbol": "./ESZ25P800:XCME"
                                },
                                {
                                    "strike-price": "4300.0",
                                    "call": "./ESZ5 ESZ5  251219C4300",
                                    "put": "./ESZ5 ESZ5  251219P4300"
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let chain: FuturesNestedOptionChain = serde_json::from_str(json).unwrap();
        
        // Verify futures array
        assert_eq!(chain.futures.len(), 1);
        assert_eq!(chain.futures[0].symbol, "/ESU5");
        assert_eq!(chain.futures[0].root_symbol, "/ES");
        assert_eq!(chain.futures[0].days_to_expiration, 18);
        assert!(chain.futures[0].active_month);
        assert!(!chain.futures[0].next_active_month);
        
        // Verify option chains array
        assert_eq!(chain.option_chains.len(), 1);
        assert_eq!(chain.option_chains[0].underlying_symbol, "/ES");
        assert_eq!(chain.option_chains[0].exercise_style, "American");
        
        // Verify expirations
        assert_eq!(chain.option_chains[0].expirations.len(), 1);
        let expiration = &chain.option_chains[0].expirations[0];
        assert_eq!(expiration.underlying_symbol, "/ESZ5");
        assert_eq!(expiration.days_to_expiration, 109);
        
        // Verify tick sizes
        assert_eq!(expiration.tick_sizes.len(), 2);
        assert_eq!(expiration.tick_sizes[0].threshold, Some("10.0".to_string()));
        assert_eq!(expiration.tick_sizes[0].value, "0.05");
        assert_eq!(expiration.tick_sizes[1].threshold, None);
        assert_eq!(expiration.tick_sizes[1].value, "0.25");
        
        // Verify strikes
        assert_eq!(expiration.strikes.len(), 2);
        assert_eq!(expiration.strikes[0].strike_price, Decimal::from_str("800.0").unwrap());
        assert_eq!(expiration.strikes[0].call, "./ESZ5 ESZ5  251219C800");
        assert_eq!(expiration.strikes[0].call_streamer_symbol, Some("./ESZ25C800:XCME".to_string()));
        
        // Second strike without streamer symbols
        assert_eq!(expiration.strikes[1].strike_price, Decimal::from_str("4300.0").unwrap());
        assert_eq!(expiration.strikes[1].call_streamer_symbol, None);
        assert_eq!(expiration.strikes[1].put_streamer_symbol, None);
    }
}
