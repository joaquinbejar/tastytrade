use crate::streaming::account_streaming::AccountEvent;

/// Represents events originating from different data feeds.
#[allow(dead_code)]
#[derive(Debug)]
pub enum TastyEvent {
    /// Represents an event from the quote feed.
    QuoteFeed(dxfeed::Event),
    /// Represents an event from the account feed.
    AccountFeed(AccountEvent),
}
