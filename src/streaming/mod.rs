/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 5/3/25
******************************************************************************/

/// DXLink market-data streaming: subscriptions and per-symbol routing.
pub mod quote_streamer;

/// Bounded reconnection policy shared by both streamers.
pub mod reconnect;

/// The account websocket: balances, orders and positions.
pub mod account_streaming;
