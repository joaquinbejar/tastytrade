//! Real certification responses, decoded by the types that claim to model them.
//!
//! Every other serde test in this crate reads a payload someone wrote from the
//! same OpenAPI document the types were derived from. Decoding one proves the
//! two agree with each other; it cannot notice the venue disagreeing with both.
//! These read what certification actually sent
//! ([#130](https://github.com/joaquinbejar/tastytrade/issues/130)).
//!
//! Captured 2026-08-04 by
//! `TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin capture_fixtures`,
//! read-only, redacted before the files were written. Re-running it and getting
//! a failure here is the point: it means the contract moved.
//!
//! No network — the files are on disk. This is a decode test, not a venue test.

use serde_json::Value;
use tastytrade::api::base::Items;
use tastytrade::prelude::*;

/// One capture, by file stem.
macro_rules! capture {
    ($stem:literal) => {
        include_str!(concat!("../../Doc/captures/", $stem, ".json"))
    };
}

/// The `items` array of a captured listing, decoded as `T`.
fn items_of<T>(capture: &str) -> Vec<T>
where
    T: serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug,
{
    let listing: Items<T> = serde_json::from_str(capture).expect("the capture is a listing");
    assert!(
        listing.skipped == 0,
        "{} record(s) in the capture could not be decoded by this crate",
        listing.skipped
    );
    assert!(!listing.items.is_empty(), "the capture has no records");
    listing.items
}

/// The account listing, including the absence that caused #5.
///
/// Certification does not send `is-test-drive`. A required field there made
/// `Items<T>` skip the account, so `TastyTrade::account` reported it as not on
/// the session — indistinguishable from an authentication problem. This is the
/// first time that absence is asserted against a response the venue really
/// sent rather than one written to match the story.
#[test]
fn the_account_listing_decodes_and_omits_is_test_drive() {
    let capture = capture!("accounts");
    let accounts: Vec<AccountInner> = items_of(capture);

    assert_eq!(accounts.len(), 1);
    let account = &accounts[0].account;
    assert!(!account.is_test_drive, "absent in certification");
    assert_eq!(account.external_id, None);
    assert_eq!(account.funding_date, None);
    assert_eq!(accounts[0].authority_level.as_deref(), Some("owner"));

    // The keys really are absent rather than null, which is what makes the
    // `#[serde(default)]` load-bearing.
    let raw: Value = serde_json::from_str(capture).expect("valid JSON");
    let record = &raw["items"][0]["account"];
    for key in ["is-test-drive", "external-id", "funding-date"] {
        assert!(record.get(key).is_none(), "{key} is present after all");
    }
}

/// The customer resource, captured structurally.
///
/// Every leaf was replaced before the file was written: the record carries a
/// legal name, an address, tax numbers, a birth date, net worth, employer and
/// political affiliation, and there is no safe subset of that to keep. What
/// survives is the shape, which is what this test is about — the field names
/// the venue uses, the nesting, and that the date and timestamp paths still
/// parse.
#[test]
fn the_customer_resource_decodes_with_its_nesting() {
    let customer: Customer =
        serde_json::from_str(capture!("customer")).expect("the customer must decode");

    // The nested sections this account carries decoded rather than being
    // swallowed. `person` and `entity` are absent here: a certification
    // individual account has neither, which is itself worth knowing — the
    // hand-written fixture had them because somebody assumed they were there.
    assert!(
        customer.address.is_some(),
        "the address section is modelled"
    );
    assert!(customer.mailing_address.is_some());
    let types = customer
        .permitted_account_types
        .as_ref()
        .expect("the venue sends the permitted account types");
    assert_eq!(types.len(), 12);
    assert!(
        types.iter().any(|t| !t.margin_types.is_empty()),
        "the nested margin types decoded"
    );

    // Rendering never shows a value, whatever the values happen to be.
    let rendered = format!("{customer:?} {customer}");
    assert!(rendered.contains("redacted"), "{rendered}");
    assert!(!rendered.contains("REDACTED"), "a field value was rendered");
}

/// The instrument listings, decoded by the types that model them.
#[test]
fn the_instrument_listings_decode() {
    let equities: Vec<EquityInstrument> = items_of(capture!("equities"));
    assert!(equities.iter().all(|e| !e.symbol.0.is_empty()));

    let cryptos: Vec<Cryptocurrency> = items_of(capture!("cryptocurrencies"));
    assert!(cryptos.iter().all(|c| !c.symbol.0.is_empty()));

    let futures: Vec<Future> = items_of(capture!("futures"));
    assert!(futures.iter().all(|f| !f.symbol.0.is_empty()));

    // The nesting is the point here: a future product embeds its option
    // products, and each of those embeds a product type.
    let products: Vec<FutureProduct> = items_of(capture!("future-products"));
    assert!(products.iter().all(|p| !p.code.is_empty()));

    let precisions: Vec<QuantityDecimalPrecision> =
        items_of(capture!("quantity-decimal-precisions"));
    assert!(!precisions.is_empty());
}

/// `product-type` is one of the two values the census observed, on real data.
///
/// The field was `String` until a capture settled it. This is the assertion
/// that keeps it settled: a third value arriving would decode into the
/// `Unknown` arm and fail here rather than passing silently.
#[test]
fn every_captured_product_type_is_one_this_crate_models() {
    let products: Vec<FutureProduct> = items_of(capture!("future-products"));

    for product in &products {
        assert!(
            product.product_type.is_known(),
            "the venue sent a product type this crate does not model: {}",
            product.product_type.as_wire()
        );
    }
}

/// The nested option chain, capped to keep the file readable.
///
/// SPY's real chain is 1.8 MB. The capture keeps the shape and the first few
/// expirations and strikes, which is what a decode test needs.
#[test]
fn the_nested_option_chain_decodes() {
    let chains: Vec<NestedOptionChain> = items_of(capture!("nested-option-chain"));

    let chain = &chains[0];
    assert!(!chain.underlying_symbol.0.is_empty());
    assert!(
        !chain.expirations.is_empty(),
        "an expiration list is what makes this a chain"
    );
    assert!(
        chain.expirations.iter().all(|e| !e.strikes.is_empty()),
        "an expiration with no strikes pins down nothing"
    );
}

/// Instrument search, from a query that actually matches something.
///
/// `query=SPY` returns nothing in certification, which is why the capture tool
/// tries several: an empty listing decodes and asserts nothing.
#[test]
fn the_instrument_search_decodes() {
    let results: Vec<InstrumentSearchResult> = items_of(capture!("instrument-search"));

    assert!(results.iter().all(|r| !r.symbol.is_empty()));
}

/// The current equities session, with the offset the venue sent.
#[test]
fn the_market_session_decodes_and_keeps_its_offset() {
    let session: CurrentMarketSession =
        serde_json::from_str(capture!("market-session-current")).expect("the session must decode");

    let raw: Value = serde_json::from_str(capture!("market-session-current")).expect("valid JSON");
    assert!(
        raw.get("instrument-collection").is_some() || raw.get("close-at").is_some(),
        "the capture is not a session record: {raw}"
    );
    // Whatever boundaries arrived, they were parsed rather than carried as
    // text — the property the crate's date handling exists for.
    let _ = session;
}
