/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

/// The US market close, approximated as 21:00 UTC.
///
/// Built once from constants rather than per call, so there is no fallible
/// construction on the path at all.
fn market_close() -> NaiveTime {
    NaiveTime::from_hms_opt(21, 0, 0).unwrap_or(NaiveTime::MIN)
}

/// Parse expiration date string to `DateTime<Utc>`
pub fn parse_expiration_date(date_str: &str, fallback: DateTime<Utc>) -> DateTime<Utc> {
    // Try to parse the date string (format might be "2024-12-20" or similar)
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) => expiration_instant(date),
        // If parsing fails, use fallback
        Err(_) => fallback,
    }
}

/// The instant an expiration date is treated as expiring at.
///
/// Deserialization already turns the wire value into a [`NaiveDate`], so the
/// string parsing in [`parse_expiration_date`] has nothing left to do on that
/// path. This is the same convention — the US market close, approximated as
/// 21:00 UTC — applied to a date that is already a date.
pub fn expiration_instant(date: NaiveDate) -> DateTime<Utc> {
    // and_time is total, unlike and_hms_opt: there is no None to fall back
    // from, so no path where a valid date silently becomes year zero.
    date.and_time(market_close()).and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old implementation used `and_hms_opt(...).unwrap_or_default()`,
    /// which would have turned a valid date into year zero rather than
    /// failing. `and_time` is total, so that branch no longer exists.
    #[test]
    fn an_expiration_lands_at_the_market_close_on_its_own_day() {
        let date = NaiveDate::from_ymd_opt(2025, 9, 19).unwrap();
        let instant = expiration_instant(date);

        assert_eq!(instant.date_naive(), date, "the day must not shift");
        assert_eq!(instant.to_rfc3339(), "2025-09-19T21:00:00+00:00");
    }

    #[test]
    fn a_parseable_string_agrees_with_the_typed_path() {
        let fallback = DateTime::from_timestamp(0, 0).unwrap();
        let date = NaiveDate::from_ymd_opt(2025, 9, 19).unwrap();

        assert_eq!(
            parse_expiration_date("2025-09-19", fallback),
            expiration_instant(date)
        );
    }

    #[test]
    fn an_unparseable_string_uses_the_fallback() {
        let fallback = DateTime::from_timestamp(0, 0).unwrap();
        assert_eq!(parse_expiration_date("19/09/2025", fallback), fallback);
    }
}
