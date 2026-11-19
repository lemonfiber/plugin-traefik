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

include!(concat!(env!("OUT_DIR"), "/fronted.rs"));

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
