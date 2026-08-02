/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 5/3/25
******************************************************************************/

/// DXLink market-data streaming: subscriptions and per-symbol routing.
pub mod quote_streamer;

/// Bounded reconnection policy.
///
/// Used by [`account_streaming`] today. The quote streamer has no reconnect
/// path yet: its DXLink client is moved into its command task, so reaching it
/// again is a change to that ownership rather than a use of this module.
pub mod reconnect;

/// The account websocket: balances, orders and positions.
pub mod account_streaming;
