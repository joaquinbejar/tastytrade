use crate::streaming::account_streaming::AccountEvent;

/// Represents events originating from different data feeds.
#[derive(Debug)]
#[allow(dead_code)]
pub enum TastyEvent {
    /// Represents an event from the quote feed.
    QuoteFeed(crate::types::dxfeed::Event),
    /// Represents an event from the account feed.
    AccountFeed(AccountEvent),
}
