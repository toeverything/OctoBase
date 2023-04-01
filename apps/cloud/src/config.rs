use chrono::Duration;
use std::env;

/// A config of [`Context`].
#[derive(Clone, Debug)]
pub struct Config {
    /// 60 seconds by default.
    pub access_token_expire_time: Duration,
    /// 180 days by default.
    pub refresh_token_expire_time: Duration,
}

impl Config {
    pub fn new() -> Self {
        let access_token_expire_time = Duration::seconds(
            env::var("JWT_ACCESS_TOKEN_EXPIRE_SECONDS")
                .ok()
                .and_then(|val| val.parse::<i64>().ok())
                .unwrap_or(60),
        );

        let refresh_token_expire_time = Duration::seconds(
            env::var("JWT_REFRESH_TOKEN_EXPIRE_SECONDS")
                .ok()
                .and_then(|val| val.parse::<i64>().ok())
                .unwrap_or(180 * 86400),
        );

        Self {
            access_token_expire_time,
            refresh_token_expire_time,
        }
    }
}
