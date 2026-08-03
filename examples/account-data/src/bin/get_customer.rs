//! The full customer resource, printed without printing any of it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin get_customer
//! ```
//!
//! This is the most sensitive object in the API — names, home address, tax
//! identifiers, birth date, net worth — so the example shows **which fields
//! arrived**, never what is in them. `Customer` will not render its contents
//! even if asked; reading a value takes naming the field, which is a decision
//! rather than an accident.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let customer = tasty.customer().await?;

    // Safe to print: the type renders as a field count.
    info!("Customer resource: {customer:?}");

    // Structure, not content. Which sections came back is useful to know and
    // says nothing about the person.
    info!(
        "  address: {}",
        present(
            customer
                .address
                .as_ref()
                .map(CustomerAddress::populated_fields)
        )
    );
    info!(
        "  mailing address: {}",
        present(
            customer
                .mailing_address
                .as_ref()
                .map(CustomerAddress::populated_fields)
        )
    );
    info!(
        "  suitability: {}",
        present(
            customer
                .customer_suitability
                .as_ref()
                .map(CustomerSuitability::populated_fields)
        )
    );
    info!(
        "  person: {}",
        present(
            customer
                .person
                .as_ref()
                .map(CustomerPerson::populated_fields)
        )
    );
    info!(
        "  entity: {}",
        present(
            customer
                .entity
                .as_ref()
                .map(CustomerEntity::populated_fields)
        )
    );

    // Two non-identifying flags, which is about the limit of what can be shown.
    info!("  professional: {:?}", customer.is_professional);
    info!("  agreed to margining: {:?}", customer.agreed_to_margining);

    // `find_customer` sends the venue's `allow-missing`, so a customer that is
    // not there is an ordinary `None` rather than a 404.
    match tasty
        .find_customer("00000000-0000-0000-0000-000000000000")
        .await?
    {
        Some(_) => info!("A customer answered for a made-up id, which is surprising"),
        None => info!("A made-up customer id resolves to None rather than an error"),
    }

    Ok(())
}

/// How many fields a section carried, or that it was absent.
fn present(fields: Option<usize>) -> String {
    match fields {
        Some(count) => format!("{count} field(s)"),
        None => "absent".to_string(),
    }
}
