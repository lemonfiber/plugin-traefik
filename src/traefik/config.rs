//! A routing table as the dynamic configuration Traefik's file provider reads, each value held
//! to its grammar before anything is written.

use std::net::Ipv4Addr;

use lemonfiber_ports::service::{Route, Routing, Tls};
use serde_json::{json, Map, Value};

/// Where Traefik's configuration directory is mounted inside its own container.
pub const TRAEFIK_CONFIGURATION: &str = "/etc/traefik";

/// The file Traefik reads its starting configuration from.
pub const STARTING: &str = "traefik.toml";

/// The file Traefik's file provider watches.
pub const DYNAMIC: &str = "dynamic.toml";

/// The table the dynamic configuration was written from.
pub const TABLE: &str = "routing.json";

/// The files in the configuration directory a certificate may never be read from.
pub const OWN: [&str; 3] = [STARTING, DYNAMIC, TABLE];

/// The HTTPS entry point every route answers on.
pub const ENTRY_POINT: &str = "websecure";

const ROUTE: &str = "route";
const STORE: &str = "default";
const LONGEST_NAME: usize = 253;
const LONGEST_LABEL: usize = 63;
const DEEPEST_FILE: usize = 4;
const LONGEST_SEGMENT: usize = 64;

/// Why a table was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A certificate or key that is not a plain file beneath the configuration directory.
    Outside(String),
    /// A name that is not a DNS name of letters, digits and hyphens, a wildcard included.
    Unnamed(String),
    /// A name two routes of the table answer on.
    Twice(String),
    /// A host that is not a DNS name or an IPv4 address.
    Unhosted(String),
    /// A port that no service answers on.
    Unported(u16),
}

/// The dynamic configuration Traefik serves `table` from.
///
/// # Errors
///
/// [`Refused`] naming the first value that is not of its grammar.
pub fn rendered(table: &Routing) -> Result<Value, Refused> {
    let mut seen = std::collections::BTreeSet::new();
    for route in &table.routes {
        if !seen.insert(route.name.to_ascii_lowercase()) {
            return Err(Refused::Twice(route.name.clone()));
        }
    }
    let mut routers = Map::new();
    let mut services = Map::new();
    for (place, one) in table.routes.iter().enumerate() {
        let id = format!("{ROUTE}-{place}");
        let (router, service) = route(one, &id)?;
        routers.insert(id.clone(), router);
        services.insert(id, service);
    }
    let mut configuration = Map::new();
    if !routers.is_empty() {
        configuration.insert(
            "http".to_owned(),
            json!({ "routers": routers, "services": services }),
        );
    }
    if let Tls::Files { certificate, key } = &table.tls {
        configuration.insert(
            "tls".to_owned(),
            json!({ "stores": { STORE: { "defaultCertificate": {
                "certFile": beneath(certificate)?,
                "keyFile": beneath(key)?,
            } } } }),
        );
    }
    Ok(Value::Object(configuration))
}

fn route(route: &Route, id: &str) -> Result<(Value, Value), Refused> {
    if route.port == 0 {
        return Err(Refused::Unported(route.port));
    }
    let name = named(&route.name).ok_or_else(|| Refused::Unnamed(route.name.clone()))?;
    let host = hosted(&route.host).ok_or_else(|| Refused::Unhosted(route.host.clone()))?;
    Ok((
        json!({
            "rule": format!("Host(`{name}`)"),
            "entryPoints": [ENTRY_POINT],
            "service": id,
            "tls": {},
        }),
        json!({ "loadBalancer": { "servers": [{ "url": format!("http://{host}:{}", route.port) }] } }),
    ))
}

fn named(name: &str) -> Option<&str> {
    dns(name).then_some(name)
}

fn hosted(host: &str) -> Option<&str> {
    (dns(host) || host.parse::<Ipv4Addr>().is_ok()).then_some(host)
}

fn dns(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= LONGEST_NAME
        && !name
            .rsplit('.')
            .next()
            .is_some_and(|last| last.bytes().all(|byte| byte.is_ascii_digit()))
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= LONGEST_LABEL
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn beneath(file: &str) -> Result<String, Refused> {
    let segments: Vec<&str> = file.split('/').collect();
    let plain = !file.is_empty()
        && segments.len() <= DEEPEST_FILE
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.len() <= LONGEST_SEGMENT
                && !segment.starts_with('.')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        && !OWN.contains(&file);
    if plain {
        Ok(format!("{TRAEFIK_CONFIGURATION}/{file}"))
    } else {
        Err(Refused::Outside(file.to_owned()))
    }
}

#[cfg(test)]
mod tests;
