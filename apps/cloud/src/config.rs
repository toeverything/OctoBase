use chrono::Duration;
use std::env;

/// A config of `Context`.
#[derive(Clone, Debug)]
pub struct Config {
    /// 60 seconds by default.
    pub access_token_expires_in: Duration,
    /// 180 days by default.
    pub refresh_token_expires_in: Duration,
}

impl Config {
    pub fn new() -> Self {
        let access_token_expires_in = Duration::seconds(
            env::var("JWT_ACCESS_TOKEN_EXPIRES_IN")
                .ok()
                .and_then(parse)
                .unwrap_or(60),
        );

        let refresh_token_expires_in = Duration::seconds(
            env::var("JWT_REFRESH_TOKEN_EXPIRES_IN")
                .ok()
                .and_then(parse)
                .unwrap_or(180 * 86400),
        );

        Self {
            access_token_expires_in,
            refresh_token_expires_in,
        }
    }
}

fn parse(val: String) -> Option<i64> {
    if val.is_empty() {
        None
    } else {
        val.parse::<i64>().ok()
    }
}
