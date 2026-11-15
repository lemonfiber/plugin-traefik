//! Traefik, told the routing table it serves through the dynamic configuration its file
//! provider watches, which it takes up without a restart.

pub mod config;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lemonfiber_adapter_kit::shared::{Shared, SharedName, Unshared};
use lemonfiber_ports::service::{Applied, Failure, Proxying, Routing, Tls};
use serde_json::Value;

use self::config::{Refused, DYNAMIC, TABLE};

const SERVICE: &str = "Traefik";
const MODE: u32 = 0o644;

/// One Traefik, told what to serve through its configuration directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Traefik {
    directory: PathBuf,
}

impl Traefik {
    /// The Traefik whose configuration directory is shared at `directory`.
    #[must_use]
    pub fn at(directory: &Path) -> Self {
        Self {
            directory: directory.to_path_buf(),
        }
    }

    /// Whether Traefik's configuration directory is there to write.
    pub async fn answers(&self) -> bool {
        tokio::fs::metadata(&self.directory)
            .await
            .is_ok_and(|held| held.is_dir())
    }

    async fn read(&self, file: &str) -> Result<Option<Vec<u8>>, Failure> {
        Shared::at(&self.directory)
            .read(&SharedName::new(file).map_err(unshared)?)
            .await
            .map_err(unshared)
    }

    async fn put(&self, file: &str, contents: Vec<u8>) -> Result<(), Failure> {
        Shared::at(&self.directory)
            .write(&SharedName::new(file).map_err(unshared)?, contents, MODE)
            .await
            .map_err(unshared)
    }

    async fn written(&self) -> Result<Option<Routing>, Failure> {
        self.read(TABLE)
            .await?
            .map(|bytes| {
                serde_json::from_slice(&bytes)
                    .map_err(|_| refused("the table written could not be read"))
            })
            .transpose()
    }

    async fn configuration(&self) -> Result<Value, Failure> {
        let bytes = self
            .read(DYNAMIC)
            .await?
            .ok_or_else(|| refused("its configuration is not there"))?;
        String::from_utf8(bytes)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .ok_or_else(|| refused("its configuration could not be read"))
    }
}

#[async_trait]
impl Proxying for Traefik {
    async fn apply(&self, table: &Routing) -> Result<Applied, Failure> {
        let configuration = rendered(table)?;
        self.put(TABLE, serde_json::to_vec(table).unwrap_or_default())
            .await?;
        self.put(DYNAMIC, serialized(&configuration)).await?;
        if &self.routing().await? == table {
            Ok(Applied::Now)
        } else {
            Err(refused("the table written reads back as another"))
        }
    }

    async fn routing(&self) -> Result<Routing, Failure> {
        let Some(table) = self.written().await? else {
            return Ok(Routing {
                tls: Tls::Automatic,
                routes: Vec::new(),
            });
        };
        let rendered = rendered(&table)?;
        if self.configuration().await? == rendered {
            Ok(table)
        } else {
            Err(refused(
                "its configuration is not the one written from the table",
            ))
        }
    }

    async fn reload(&self) -> Result<Applied, Failure> {
        if let Some(table) = self.written().await? {
            self.put(DYNAMIC, serialized(&rendered(&table)?)).await?;
        }
        self.routing().await.map(|_| Applied::Now)
    }
}

fn rendered(table: &Routing) -> Result<Value, Failure> {
    config::rendered(table).map_err(|why| refused(&said(&why)))
}

fn serialized(configuration: &Value) -> Vec<u8> {
    toml::to_string(configuration)
        .unwrap_or_default()
        .into_bytes()
}

fn said(why: &Refused) -> String {
    match why {
        Refused::Outside(file) => {
            format!("{file:?} is not a plain file beneath the proxy's configuration directory")
        }
        Refused::Unnamed(name) => format!("{name:?} is not a plain DNS name"),
        Refused::Twice(name) => format!("{name:?} is the name of two routes"),
        Refused::Unhosted(host) => format!("{host:?} is not a DNS name or an IPv4 address"),
        Refused::Unported(port) => format!("{port} is not a port a service answers on"),
    }
}

fn refused(detail: &str) -> Failure {
    Failure::Refused {
        service: SERVICE.to_owned(),
        detail: detail.to_owned(),
    }
}

fn unshared(why: Unshared) -> Failure {
    match why {
        Unshared::Failed(..) => Failure::Unavailable {
            service: SERVICE.to_owned(),
        },
        refusal => refused(&refusal.to_string()),
    }
}

#[cfg(test)]
mod tests;
