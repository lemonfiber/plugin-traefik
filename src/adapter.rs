//! What this adapter says it is, and the kit that serves its contract.

use std::sync::Arc;

use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::proxy;

use crate::traefik::Traefik;

/// Where Traefik's configuration directory is shared, in the adapter's container.
pub const ADDRESS: &str = "/traefik";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Traefik";

/// The Traefik release this adapter's recordings were taken from.
pub const RELEASE: &str = "v3.7.19";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:aa942a00bce033c909bf1f7e861d93761be4b31a035f10d2410d33e38a8f41cf";

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            proxy::front::CAPABILITY,
            proxy::front::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The contract, served from `traefik` under `key`, ready while Traefik's configuration
/// directory is there to write.
#[must_use]
pub fn kit(key: String, traefik: Arc<Traefik>) -> Kit {
    let asked = Arc::clone(&traefik);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(proxy::front::served(traefik))
}

#[cfg(test)]
mod tests;
