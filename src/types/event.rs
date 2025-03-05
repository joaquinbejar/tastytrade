use crate::streaming::account_streaming::AccountEvent;

#[allow(dead_code)]
#[derive(Debug)]
pub enum TastyEvent {
    QuoteFeed(dxfeed::Event),
    AccountFeed(AccountEvent),
}
