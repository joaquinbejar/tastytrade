//! The customer resource: the most sensitive object this crate touches.
//!
//! Names, home and mailing addresses, tax identifiers, birth dates, visa
//! status, net worth, employer, political affiliation, family member names.
//! Every one of these is personal data about a real person, and several of
//! them are identity-theft material on their own.
//!
//! So the rule here is stricter than "keep it out of logs": **nothing in this
//! module renders itself**. `Debug` and `Display` print the type name and how
//! many fields arrived, never a value — the same treatment
//! [`crate::prelude::RawPayload`] gets, for the same reason. Reading a value
//! means naming the field, which is a decision a caller makes on purpose and a
//! reviewer can grep for.
//!
//! `Serialize` is derived because the generic verbs require it. Serialising a
//! customer writes the personal data, which is correct — it is an explicit act.

use std::fmt;

use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Writes `Debug` and `Display` impls that render nothing but a field count.
///
/// A macro rather than seven hand-written impls: a per-field redaction is a
/// per-field opportunity to forget one, and the venue adds fields. This cannot
/// leak a value it has never been told about.
macro_rules! redacted_debug {
    ($name:ident) => {
        impl $name {
            /// How many fields the venue actually sent.
            ///
            /// Structural, not personal: a count is safe to print and is the
            /// only thing `Debug` is allowed to say about the contents.
            pub fn populated_fields(&self) -> usize {
                populated(self)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    concat!(stringify!($name), "(<redacted, {} field(s) present>)"),
                    self.populated_fields()
                )
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "<redacted {}>", stringify!($name))
            }
        }
    };
}

/// How many fields a value actually carries.
///
/// Counted by serialising and counting the keys serde emits, so it stays right
/// when a field is added and cannot itself print a value. Structural, not
/// personal.
fn populated(value: &impl Serialize) -> usize {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| match value {
            serde_json::Value::Object(map) => Some(map.values().filter(|v| !v.is_null()).count()),
            _ => None,
        })
        .unwrap_or(0)
}

/// One account type this customer is permitted to open.
///
/// **Snake case, alone in this module.** Every other object the customer
/// resource carries is kebab-case; this one and its margin types are not, which
/// is why they carry explicit renames instead of a container rule. Observed in
/// the 2026-08-04 capture — the field was typed `String` before that, which
/// made the whole customer resource fail to decode against the real venue.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PermittedAccountType {
    /// The venue's name for the account type.
    #[serde(default)]
    pub name: Option<String>,
    /// A longer description of it.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the type admits more than one owner.
    #[serde(default)]
    pub has_multiple_owners: Option<bool>,
    /// Whether it is offered publicly.
    #[serde(default)]
    pub is_publicly_available: Option<bool>,
    /// Whether it carries a tax advantage.
    #[serde(default)]
    pub is_tax_advantaged: Option<bool>,
    /// The margin arrangements it supports.
    #[serde(default)]
    pub margin_types: Vec<PermittedMarginType>,
}

redacted_debug!(PermittedAccountType);

/// One margin arrangement a [`PermittedAccountType`] supports.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PermittedMarginType {
    /// The venue's name for it.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether it is a margin arrangement rather than a cash one.
    #[serde(default)]
    pub is_margin: Option<bool>,
}

redacted_debug!(PermittedMarginType);

/// A postal address.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CustomerAddress {
    /// City.
    #[serde(default)]
    pub city: Option<String>,
    /// Country.
    #[serde(default)]
    pub country: Option<String>,
    /// Is domestic.
    #[serde(default)]
    pub is_domestic: Option<bool>,
    /// Is foreign.
    #[serde(default)]
    pub is_foreign: Option<bool>,
    /// Postal code.
    #[serde(default)]
    pub postal_code: Option<String>,
    /// State region.
    #[serde(default)]
    pub state_region: Option<String>,
    /// Street one.
    #[serde(default)]
    pub street_one: Option<String>,
    /// Street three.
    #[serde(default)]
    pub street_three: Option<String>,
    /// Street two.
    #[serde(default)]
    pub street_two: Option<String>,
}

redacted_debug!(CustomerAddress);

/// The customer's declared finances and trading experience.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CustomerSuitability {
    /// Id.
    #[serde(default)]
    pub id: Option<String>,
    /// Annual net income.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub annual_net_income: Option<Decimal>,
    /// Covered options trading experience.
    #[serde(default)]
    pub covered_options_trading_experience: Option<String>,
    /// Customer id.
    #[serde(default)]
    pub customer_id: Option<i64>,
    /// Employer name.
    #[serde(default)]
    pub employer_name: Option<String>,
    /// Employment status.
    #[serde(default)]
    pub employment_status: Option<String>,
    /// Futures trading experience.
    #[serde(default)]
    pub futures_trading_experience: Option<String>,
    /// Job title.
    #[serde(default)]
    pub job_title: Option<String>,
    /// Liquid net worth.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub liquid_net_worth: Option<Decimal>,
    /// Marital status.
    #[serde(default)]
    pub marital_status: Option<String>,
    /// Net worth.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub net_worth: Option<Decimal>,
    /// Number of dependents.
    #[serde(default)]
    pub number_of_dependents: Option<i64>,
    /// Occupation.
    #[serde(default)]
    pub occupation: Option<String>,
    /// Stock trading experience.
    #[serde(default)]
    pub stock_trading_experience: Option<String>,
    /// Tax bracket.
    #[serde(default)]
    pub tax_bracket: Option<String>,
    /// Uncovered options trading experience.
    #[serde(default)]
    pub uncovered_options_trading_experience: Option<String>,
}

redacted_debug!(CustomerSuitability);

/// The natural person behind a customer.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CustomerPerson {
    /// External id.
    #[serde(default)]
    pub external_id: Option<String>,
    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,
    /// Middle name.
    #[serde(default)]
    pub middle_name: Option<String>,
    /// Prefix name.
    #[serde(default)]
    pub prefix_name: Option<String>,
    /// Suffix name.
    #[serde(default)]
    pub suffix_name: Option<String>,
    /// Birth country.
    #[serde(default)]
    pub birth_country: Option<String>,
    /// Birth date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub birth_date: Option<NaiveDate>,
    /// Citizenship country.
    #[serde(default)]
    pub citizenship_country: Option<String>,
    /// Usa citizenship type.
    #[serde(default)]
    pub usa_citizenship_type: Option<String>,
    /// Visa expiration date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub visa_expiration_date: Option<NaiveDate>,
    /// Visa type.
    #[serde(default)]
    pub visa_type: Option<String>,
    /// Employer name.
    #[serde(default)]
    pub employer_name: Option<String>,
    /// Employment status.
    #[serde(default)]
    pub employment_status: Option<String>,
    /// Job title.
    #[serde(default)]
    pub job_title: Option<String>,
    /// Marital status.
    #[serde(default)]
    pub marital_status: Option<String>,
    /// Number of dependents.
    #[serde(default)]
    pub number_of_dependents: Option<i64>,
    /// Occupation.
    #[serde(default)]
    pub occupation: Option<String>,
}

redacted_debug!(CustomerPerson);

/// An officer of an entity customer.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct EntityOfficer {
    /// Id.
    #[serde(default)]
    pub id: Option<String>,
    /// External id.
    #[serde(default)]
    pub external_id: Option<String>,
    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,
    /// Middle name.
    #[serde(default)]
    pub middle_name: Option<String>,
    /// Prefix name.
    #[serde(default)]
    pub prefix_name: Option<String>,
    /// Suffix name.
    #[serde(default)]
    pub suffix_name: Option<String>,
    /// Address.
    #[serde(default)]
    pub address: Option<CustomerAddress>,
    /// Birth country.
    #[serde(default)]
    pub birth_country: Option<String>,
    /// Birth date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub birth_date: Option<NaiveDate>,
    /// Citizenship country.
    #[serde(default)]
    pub citizenship_country: Option<String>,
    /// Email.
    #[serde(default)]
    pub email: Option<String>,
    /// Employer name.
    #[serde(default)]
    pub employer_name: Option<String>,
    /// Employment status.
    #[serde(default)]
    pub employment_status: Option<String>,
    /// Home phone number.
    #[serde(default)]
    pub home_phone_number: Option<String>,
    /// Is foreign.
    #[serde(default)]
    pub is_foreign: Option<bool>,
    /// Job title.
    #[serde(default)]
    pub job_title: Option<String>,
    /// Marital status.
    #[serde(default)]
    pub marital_status: Option<String>,
    /// Mobile phone number.
    #[serde(default)]
    pub mobile_phone_number: Option<String>,
    /// Number of dependents.
    #[serde(default)]
    pub number_of_dependents: Option<i64>,
    /// Occupation.
    #[serde(default)]
    pub occupation: Option<String>,
    /// Owner of record.
    #[serde(default)]
    pub owner_of_record: Option<bool>,
    /// Relationship to entity.
    #[serde(default)]
    pub relationship_to_entity: Option<String>,
    /// Tax number.
    #[serde(default)]
    pub tax_number: Option<String>,
    /// Tax number type.
    #[serde(default)]
    pub tax_number_type: Option<String>,
    /// Usa citizenship type.
    #[serde(default)]
    pub usa_citizenship_type: Option<String>,
    /// Visa expiration date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub visa_expiration_date: Option<NaiveDate>,
    /// Visa type.
    #[serde(default)]
    pub visa_type: Option<String>,
    /// Work phone number.
    #[serde(default)]
    pub work_phone_number: Option<String>,
}

redacted_debug!(EntityOfficer);

/// An entity's declared finances and trading experience.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct EntitySuitability {
    /// Id.
    #[serde(default)]
    pub id: Option<String>,
    /// Annual net income.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub annual_net_income: Option<Decimal>,
    /// Covered options trading experience.
    #[serde(default)]
    pub covered_options_trading_experience: Option<String>,
    /// Entity id.
    #[serde(default)]
    pub entity_id: Option<i64>,
    /// Futures trading experience.
    #[serde(default)]
    pub futures_trading_experience: Option<String>,
    /// Liquid net worth.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub liquid_net_worth: Option<Decimal>,
    /// Net worth.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub net_worth: Option<Decimal>,
    /// Stock trading experience.
    #[serde(default)]
    pub stock_trading_experience: Option<String>,
    /// Tax bracket.
    #[serde(default)]
    pub tax_bracket: Option<String>,
    /// Uncovered options trading experience.
    #[serde(default)]
    pub uncovered_options_trading_experience: Option<String>,
}

redacted_debug!(EntitySuitability);

/// The legal entity behind a customer, when there is one.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CustomerEntity {
    /// Id.
    #[serde(default)]
    pub id: Option<String>,
    /// Address.
    #[serde(default)]
    pub address: Option<CustomerAddress>,
    /// Business nature.
    #[serde(default)]
    pub business_nature: Option<String>,
    /// Date of trust creation.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub date_of_trust_creation: Option<NaiveDate>,
    /// Email.
    #[serde(default)]
    pub email: Option<String>,
    /// Entity officers.
    pub entity_officers: Option<Vec<EntityOfficer>>,
    /// Entity suitability.
    #[serde(default)]
    pub entity_suitability: Option<EntitySuitability>,
    /// Entity type.
    #[serde(default)]
    pub entity_type: Option<String>,
    /// Foreign institution.
    #[serde(default)]
    pub foreign_institution: Option<String>,
    /// Grantor birth date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub grantor_birth_date: Option<NaiveDate>,
    /// Grantor email.
    #[serde(default)]
    pub grantor_email: Option<String>,
    /// Grantor first name.
    #[serde(default)]
    pub grantor_first_name: Option<String>,
    /// Grantor last name.
    #[serde(default)]
    pub grantor_last_name: Option<String>,
    /// Grantor middle name.
    #[serde(default)]
    pub grantor_middle_name: Option<String>,
    /// Grantor tax number.
    #[serde(default)]
    pub grantor_tax_number: Option<String>,
    /// Has foreign bank affiliation.
    #[serde(default)]
    pub has_foreign_bank_affiliation: Option<String>,
    /// Has foreign institution affiliation.
    #[serde(default)]
    pub has_foreign_institution_affiliation: Option<String>,
    /// Is domestic.
    #[serde(default)]
    pub is_domestic: Option<bool>,
    /// Is hedge fund.
    #[serde(default)]
    pub is_hedge_fund: Option<String>,
    /// Legal name.
    #[serde(default)]
    pub legal_name: Option<String>,
    /// Mailing address.
    #[serde(default)]
    pub mailing_address: Option<CustomerAddress>,
    /// Phone number.
    #[serde(default)]
    pub phone_number: Option<String>,
    /// Secretary name.
    #[serde(default)]
    pub secretary_name: Option<String>,
    /// Tax election.
    #[serde(default)]
    pub tax_election: Option<String>,
    /// Tax number.
    #[serde(default)]
    pub tax_number: Option<String>,
}

redacted_debug!(CustomerEntity);

/// A full customer resource.
///
/// Every field is `Option<T>`: the venue sends what it holds, and a field it
/// omitted is unknown rather than empty. `Debug` and `Display` redact.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Customer {
    /// Id.
    #[serde(default)]
    pub id: Option<String>,
    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,
    /// First surname.
    #[serde(default)]
    pub first_surname: Option<String>,
    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,
    /// Middle name.
    #[serde(default)]
    pub middle_name: Option<String>,
    /// Prefix name.
    #[serde(default)]
    pub prefix_name: Option<String>,
    /// Second surname.
    #[serde(default)]
    pub second_surname: Option<String>,
    /// Suffix name.
    #[serde(default)]
    pub suffix_name: Option<String>,
    /// Cherry club date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub cherry_club_date: Option<NaiveDate>,
    /// Address.
    #[serde(default)]
    pub address: Option<CustomerAddress>,
    /// Customer suitability.
    #[serde(default)]
    pub customer_suitability: Option<CustomerSuitability>,
    /// Mailing address.
    #[serde(default)]
    pub mailing_address: Option<CustomerAddress>,
    /// Is foreign.
    #[serde(default)]
    pub is_foreign: Option<bool>,
    /// Regulatory domain.
    #[serde(default)]
    pub regulatory_domain: Option<String>,
    /// Usa citizenship type.
    #[serde(default)]
    pub usa_citizenship_type: Option<String>,
    /// Home phone number.
    #[serde(default)]
    pub home_phone_number: Option<String>,
    /// Home phone number details.
    #[serde(default)]
    pub home_phone_number_details: Option<String>,
    /// Mobile phone number.
    #[serde(default)]
    pub mobile_phone_number: Option<String>,
    /// Mobile phone number details.
    #[serde(default)]
    pub mobile_phone_number_details: Option<String>,
    /// Work phone number.
    #[serde(default)]
    pub work_phone_number: Option<String>,
    /// Work phone number details.
    #[serde(default)]
    pub work_phone_number_details: Option<String>,
    /// Birth date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub birth_date: Option<NaiveDate>,
    /// Email.
    #[serde(default)]
    pub email: Option<String>,
    /// External id.
    #[serde(default)]
    pub external_id: Option<String>,
    /// Foreign tax number.
    #[serde(default)]
    pub foreign_tax_number: Option<String>,
    /// Tax number.
    #[serde(default)]
    pub tax_number: Option<String>,
    /// Tax number type.
    #[serde(default)]
    pub tax_number_type: Option<String>,
    /// Birth country.
    #[serde(default)]
    pub birth_country: Option<String>,
    /// Citizenship country.
    #[serde(default)]
    pub citizenship_country: Option<String>,
    /// Visa expiration date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub visa_expiration_date: Option<NaiveDate>,
    /// Visa type.
    #[serde(default)]
    pub visa_type: Option<String>,
    /// Agreed to margining.
    #[serde(default)]
    pub agreed_to_margining: Option<bool>,
    /// Subject to tax withholding.
    #[serde(default)]
    pub subject_to_tax_withholding: Option<bool>,
    /// Agreed to terms.
    #[serde(default)]
    pub agreed_to_terms: Option<bool>,
    /// Signature of agreement.
    #[serde(default)]
    pub signature_of_agreement: Option<bool>,
    /// Desk customer id.
    #[serde(default)]
    pub desk_customer_id: Option<String>,
    /// Ext crm id.
    #[serde(default)]
    pub ext_crm_id: Option<String>,
    /// Family member names.
    #[serde(default)]
    pub family_member_names: Option<String>,
    /// Gender.
    #[serde(default)]
    pub gender: Option<String>,
    /// Has industry affiliation.
    #[serde(default)]
    pub has_industry_affiliation: Option<bool>,
    /// Has institutional assets.
    #[serde(default)]
    pub has_institutional_assets: Option<String>,
    /// Has listed affiliation.
    #[serde(default)]
    pub has_listed_affiliation: Option<bool>,
    /// Has political affiliation.
    #[serde(default)]
    pub has_political_affiliation: Option<bool>,
    /// Industry affiliation firm.
    #[serde(default)]
    pub industry_affiliation_firm: Option<String>,
    /// Is investment adviser.
    #[serde(default)]
    pub is_investment_adviser: Option<String>,
    /// Listed affiliation symbol.
    #[serde(default)]
    pub listed_affiliation_symbol: Option<String>,
    /// Political organization.
    #[serde(default)]
    pub political_organization: Option<String>,
    /// User id.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Has delayed quotes.
    #[serde(default)]
    pub has_delayed_quotes: Option<bool>,
    /// Has pending or approved application.
    #[serde(default)]
    pub has_pending_or_approved_application: Option<bool>,
    /// Is professional.
    #[serde(default)]
    pub is_professional: Option<bool>,
    /// Permitted account types.
    #[serde(default)]
    pub permitted_account_types: Option<Vec<PermittedAccountType>>,
    /// Created at.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub created_at: Option<DateTime<FixedOffset>>,
    /// Entity.
    #[serde(default)]
    pub entity: Option<CustomerEntity>,
    /// Identifiable type.
    #[serde(default)]
    pub identifiable_type: Option<String>,
    /// Person.
    #[serde(default)]
    pub person: Option<CustomerPerson>,
}

redacted_debug!(Customer);
