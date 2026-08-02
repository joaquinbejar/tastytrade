//! Bounded reconnection policy shared by both streamers.
//!
//! A dropped socket is normal: venues restart, networks blink, sessions
//! expire. What is not acceptable is a client that reconnects in a tight loop,
//! that reconnects forever against a venue that is refusing it, or that keeps
//! presenting a credential the venue has already rejected.

use std::time::Duration;

use crate::TastyTradeError;

/// Where a streamer is in its connection lifecycle.
///
/// Public because a caller deciding whether to show "reconnecting" or "give
/// up" needs it, and because a reconnect that happens silently is
/// indistinguishable from one that is not happening.
///
/// No variant carries a token, a credential or an account identifier, so the
/// whole value is safe to log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// The stream is live.
    Connected,
    /// The connection dropped and a retry is scheduled.
    Reconnecting {
        /// Which attempt is about to be made, counting from one.
        attempt: u32,
        /// How long until it is made.
        delay: Duration,
    },
    /// No further attempts will be made.
    Disconnected {
        /// Why, in terms a caller can act on.
        reason: String,
    },
}

impl ConnectionState {
    /// Whether the stream is currently usable.
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    /// Whether this is the end of the line.
    pub fn is_terminal(&self) -> bool {
        matches!(self, ConnectionState::Disconnected { .. })
    }
}

/// How long to wait before each attempt, and when to stop.
///
/// Exponential with a ceiling and a hard attempt limit, because unbounded
/// retries against a venue that is refusing you is how a client becomes the
/// outage.
#[derive(Debug, Clone, PartialEq)]
pub struct BackoffPolicy {
    /// Delay before the first retry.
    pub initial: Duration,
    /// The delay never grows past this, however many attempts have failed.
    pub max_delay: Duration,
    /// Give up after this many attempts. `None` retries until told to stop.
    pub max_attempts: Option<u32>,
    /// Fraction of the delay that is randomised, from `0.0` to `1.0`.
    ///
    /// Without it, every client disconnected by the same venue restart comes
    /// back at the same instant and restarts it.
    ///
    /// `f64` rather than `Decimal`: this is a proportion of a wait, not money,
    /// and nothing is ever settled in it. It is also why this type is only
    /// `PartialEq`.
    pub jitter: f64,
}

impl Default for BackoffPolicy {
    /// Sensible for a market-data socket: quick first retry, a ceiling short
    /// enough that a recovered venue is noticed promptly, and a limit that
    /// stops a client hammering a venue that is refusing it.
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            max_attempts: Some(8),
            jitter: 0.25,
        }
    }
}

impl BackoffPolicy {
    /// The delay before `attempt`, counting from one, or `None` when the
    /// policy says to stop.
    ///
    /// `now_nanos` is the jitter source. Randomness is taken from the caller
    /// rather than generated here so this is a pure function and its tests do
    /// not depend on a clock or a random number generator.
    pub fn delay_for(&self, attempt: u32, now_nanos: u64) -> Option<Duration> {
        if attempt == 0 {
            return None;
        }
        if let Some(max) = self.max_attempts
            && attempt > max
        {
            return None;
        }

        // Saturating: a long-lived connection can rack up attempts, and
        // 2^attempt overflows well before the ceiling stops mattering.
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        let base = self
            .initial
            .saturating_mul(factor)
            .min(self.max_delay)
            .as_nanos() as u64;

        if self.jitter <= 0.0 {
            return Some(Duration::from_nanos(base));
        }

        // Spread within [base * (1 - jitter), base]. Downward only, so a
        // ceiling is a ceiling.
        let span = (base as f64 * self.jitter.clamp(0.0, 1.0)) as u64;
        let offset = if span == 0 { 0 } else { now_nanos % span };
        Some(Duration::from_nanos(base.saturating_sub(offset)))
    }

    /// Whether a failure is worth retrying at all.
    ///
    /// A rejected credential does not improve on the second attempt, and a
    /// configuration mistake improves even less. Retrying either wastes the
    /// caller's time and the venue's patience.
    pub fn should_retry(&self, error: &TastyTradeError) -> bool {
        match error {
            TastyTradeError::Auth(_)
            | TastyTradeError::ConfigError(_)
            | TastyTradeError::Precondition(_) => false,
            other => other.is_retryable() || matches!(other, TastyTradeError::Streaming(_)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> BackoffPolicy {
        BackoffPolicy {
            initial: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
            max_attempts: Some(5),
            jitter: 0.0,
        }
    }

    #[test]
    fn the_delay_doubles_until_it_hits_the_ceiling() {
        let p = policy();

        assert_eq!(p.delay_for(1, 0), Some(Duration::from_millis(100)));
        assert_eq!(p.delay_for(2, 0), Some(Duration::from_millis(200)));
        assert_eq!(p.delay_for(3, 0), Some(Duration::from_millis(400)));
        // 100ms * 2^4 = 1.6s, still under the 2s ceiling.
        assert_eq!(p.delay_for(5, 0), Some(Duration::from_millis(1600)));
    }

    /// Unbounded retries against a venue that is refusing you is how a client
    /// becomes the outage.
    #[test]
    fn the_policy_stops_at_its_attempt_limit() {
        let p = policy();

        assert!(p.delay_for(5, 0).is_some(), "the last attempt is allowed");
        assert_eq!(p.delay_for(6, 0), None, "one past the limit is refused");
        assert_eq!(p.delay_for(u32::MAX, 0), None);
    }

    /// A long-lived connection can accumulate attempts, and 2^attempt
    /// overflows long before the ceiling stops mattering.
    #[test]
    fn a_huge_attempt_count_saturates_rather_than_overflowing() {
        let p = BackoffPolicy {
            max_attempts: None,
            ..policy()
        };

        assert_eq!(p.delay_for(1_000, 0), Some(Duration::from_secs(2)));
        assert_eq!(p.delay_for(u32::MAX, 0), Some(Duration::from_secs(2)));
    }

    /// Every client disconnected by the same venue restart must not come back
    /// at the same instant and restart it.
    #[test]
    fn jitter_spreads_the_delay_downward_only() {
        let p = BackoffPolicy {
            jitter: 0.5,
            ..policy()
        };

        let base = Duration::from_millis(400);
        for nanos in [0, 1, 12_345, 999_999_999, u64::MAX] {
            let delay = p.delay_for(3, nanos).expect("within the limit");
            assert!(
                delay <= base,
                "jitter must not push a delay above the ceiling: {delay:?}"
            );
            assert!(
                delay >= base / 2,
                "jitter must not collapse the delay to nothing: {delay:?}"
            );
        }
    }

    #[test]
    fn attempt_zero_is_not_a_retry() {
        assert_eq!(policy().delay_for(0, 0), None);
    }

    /// A rejected credential does not improve on the second attempt.
    #[test]
    fn authentication_and_configuration_failures_are_not_retried() {
        let p = policy();

        assert!(!p.should_retry(&TastyTradeError::Auth("rejected".into())));
        assert!(!p.should_retry(&TastyTradeError::ConfigError("missing".into())));
        assert!(!p.should_retry(&TastyTradeError::Precondition("wrong account".into())));
    }

    #[test]
    fn a_dropped_stream_is_retried() {
        let p = policy();

        assert!(p.should_retry(&TastyTradeError::Streaming("socket closed".into())));
        assert!(p.should_retry(&TastyTradeError::Connection("refused".into())));
    }

    #[test]
    fn the_state_reports_itself_without_naming_anything_private() {
        let reconnecting = ConnectionState::Reconnecting {
            attempt: 3,
            delay: Duration::from_millis(400),
        };
        assert!(!reconnecting.is_connected());
        assert!(!reconnecting.is_terminal());

        let done = ConnectionState::Disconnected {
            reason: "gave up after 8 attempts".to_string(),
        };
        assert!(done.is_terminal());

        // The rendered state is safe to log: it is built from counts and
        // durations, never from a token or an account.
        let rendered = format!("{reconnecting:?} {done:?}");
        assert!(rendered.contains("attempt: 3"));
    }
}
