use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use lemonfiber_contract::adapter::{about_path, ready_path, About};
use serde_json::json;
use tower::ServiceExt as _;

use super::{about, kit, DIGEST, RELEASE, UPSTREAM};
use crate::traefik::Traefik;

fn a_key() -> String {
    ('e'..='t').collect()
}

async fn answered(
    traefik: Arc<Traefik>,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    let Ok(request) = request.body(Body::from(body.to_owned())) else {
        return (StatusCode::IM_A_TEAPOT, Vec::new());
    };
    let Ok(response) = kit(a_key(), traefik).router().oneshot(request).await;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, body)
}

#[test]
fn it_says_it_speaks_proxy_front_for_the_traefik_its_recordings_were_taken_from() {
    let about = about();

    assert_eq!(about.speaks, vec!["proxy.front@1".to_owned()]);
    assert_eq!(about.upstream, UPSTREAM);
    assert_eq!(about.releases.len(), 1);
    assert_eq!(
        about.releases.first().map(|one| one.version.as_str()),
        Some(RELEASE)
    );
    assert_eq!(
        about.releases.first().map(|one| one.digest.as_str()),
        Some(DIGEST)
    );
}

#[tokio::test]
async fn it_names_itself_under_the_key_and_to_nobody_else() {
    let directory = Directory::new("about");
    let traefik = Arc::new(Traefik::at(&directory.0));

    let (status, body) = answered(
        Arc::clone(&traefik),
        "GET",
        &about_path(),
        Some(&a_key()),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<About>(&body).ok(), Some(about()));

    let (status, _) = answered(traefik, "GET", &about_path(), Some("another"), "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("traefik-adapter-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn it_is_ready_while_traefiks_configuration_directory_is_there() {
    let directory = Directory::new("ready");
    let traefik = Arc::new(Traefik::at(&directory.0));
    let (_, before) = answered(
        Arc::clone(&traefik),
        "GET",
        &ready_path(),
        Some(&a_key()),
        "",
    )
    .await;

    assert!(std::fs::create_dir_all(&directory.0).is_ok());
    let (_, after) = answered(traefik, "GET", &ready_path(), Some(&a_key()), "").await;

    assert_eq!(before, b"false");
    assert_eq!(after, b"true");
}

#[tokio::test]
async fn a_table_applied_over_the_contract_is_written_as_traefiks_dynamic_configuration() {
    let directory = Directory::new("apply");
    assert!(std::fs::create_dir_all(&directory.0).is_ok());
    let traefik = Arc::new(Traefik::at(&directory.0));
    let table = json!({"table": {"tls": "automatic", "routes": [
        {"name": "watch.home.example", "host": "door", "port": 8096}
    ]}});

    let (status, _) = answered(
        traefik,
        "POST",
        "/lemonfiber/proxy.front/v1/apply",
        Some(&a_key()),
        &table.to_string(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let written: toml::Table =
        std::fs::read_to_string(directory.0.join(crate::traefik::config::DYNAMIC))
            .unwrap_or_default()
            .parse()
            .unwrap_or_default();
    assert_eq!(
        written
            .get("http")
            .and_then(|http| http.get("routers"))
            .and_then(|routers| routers.get("route-0"))
            .and_then(|router| router.get("rule"))
            .and_then(toml::Value::as_str),
        Some("Host(`watch.home.example`)")
    );
}
