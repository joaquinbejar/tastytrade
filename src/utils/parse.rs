/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/
use chrono::{DateTime, Utc};

/// Parse expiration date string to `DateTime<Utc>`
pub fn parse_expiration_date(date_str: &str, fallback: DateTime<Utc>) -> DateTime<Utc> {
    // Try to parse the date string (format might be "2024-12-20" or similar)
    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        // Convert to DateTime at market close (4:00 PM ET = 21:00 UTC approximately)
        naive_date
            .and_hms_opt(21, 0, 0)
            .unwrap_or_default()
            .and_utc()
    } else {
        // If parsing fails, use fallback
        fallback
    }
}
