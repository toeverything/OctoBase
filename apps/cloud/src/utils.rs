use chrono::Duration;
use hmac::Hmac;
use sha2::Sha256;
use tokio::signal;
use tracing::info;

pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
}

#[derive(Eq, PartialEq, Debug)]
pub enum Cachability {
    /// Any cache can cache this data.
    Public,

    /// Data cannot be cached in shared caches.
    Private,

    /// No one can cache this data.
    NoCache,

    /// Cache the data the first time, and use the cache from then on.
    OnlyIfCached,
}

/// Represents a Cache-Control header
#[derive(Eq, PartialEq, Debug, Default)]
pub struct CacheControl {
    pub cachability: Option<Cachability>,
    /// The maximum amount of time a resource is considered fresh.
    /// Unlike `Expires`, this directive is relative to the time of the request.
    pub max_age: Option<Duration>,
    /// Overrides max-age or the `Expires` header, but only for shared caches (e.g., proxies).
    /// Ignored by private caches.
    pub s_max_age: Option<Duration>,
    /// Indicates the client will accept a stale response. An optional value in seconds
    /// indicates the upper limit of staleness the client will accept.
    pub max_stale: Option<Duration>,
    /// Indicates the client wants a response that will still be fresh for at least
    /// the specified number of seconds.
    pub min_fresh: Option<Duration>,
    /// Indicates that once a resource becomes stale, caches do not use their stale
    /// copy without successful validation on the origin server.
    pub must_revalidate: bool,
    /// Like `must-revalidate`, but only for shared caches (e.g., proxies).
    /// Ignored by private caches.
    pub proxy_revalidate: bool,
    /// Indicates that the response body **will not change** over time.
    pub immutable: bool,
    /// The response may not be stored in _any_ cache.
    pub no_store: bool,
    /// An intermediate cache or proxy cannot edit the response body,
    /// `Content-Encoding`, `Content-Range`, or `Content-Type`.
    pub no_transform: bool,
}

impl CacheControl {
    pub fn parse(input: &str) -> Option<Self> {
        let mut ret = Self::default();
        for token in input.split(',') {
            let (key, val) = {
                let mut split = token.split('=').map(|s| s.trim());
                (split.next().unwrap(), split.next())
            };

            match key {
                "public" => ret.cachability = Some(Cachability::Public),
                "private" => ret.cachability = Some(Cachability::Private),
                "no-cache" => ret.cachability = Some(Cachability::NoCache),
                "only-if-cached" => ret.cachability = Some(Cachability::OnlyIfCached),
                "max-age" => match val.and_then(|v| v.parse().ok()) {
                    Some(secs) => ret.max_age = Some(Duration::seconds(secs)),
                    None => return None,
                },
                "max-stale" => match val.and_then(|v| v.parse().ok()) {
                    Some(secs) => ret.max_stale = Some(Duration::seconds(secs)),
                    None => return None,
                },
                "min-fresh" => match val.and_then(|v| v.parse().ok()) {
                    Some(secs) => ret.min_fresh = Some(Duration::seconds(secs)),
                    None => return None,
                },
                "must-revalidate" => ret.must_revalidate = true,
                "proxy-revalidate" => ret.proxy_revalidate = true,
                "immutable" => ret.immutable = true,
                "no-store" => ret.no_store = true,
                "no-transform" => ret.no_transform = true,
                _ => (),
            };
        }
        Some(ret)
    }
}

pub type HmacSha256 = Hmac<Sha256>;
