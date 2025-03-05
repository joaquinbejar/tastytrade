use super::account_streaming::AccountEvent;

#[derive(Debug)]
pub enum TastyEvent {
    QuoteFeed(dxfeed::Event),
    AccountFeed(AccountEvent),
}
