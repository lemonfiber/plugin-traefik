//! The manifest and Traefik's starting files, as the core at the pinned commit reads them.

use lemonfiber_ports::service::{Routing, Tls};
use toml::{Table, Value};
use traefik_adapter::traefik::config::{
    self, DYNAMIC, ENTRY_POINT, STARTING, TRAEFIK_CONFIGURATION,
};

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_default()
}

fn manifest() -> Table {
    read("plugin.toml").parse().unwrap_or_default()
}

fn starting() -> Table {
    read(&format!("config/traefik/{STARTING}"))
        .parse()
        .unwrap_or_default()
}

fn services(manifest: &Table) -> impl Iterator<Item = &Table> {
    manifest
        .get("service")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
}

fn service(manifest: &Table, id: &str) -> Table {
    services(manifest)
        .find(|service| service.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
        .unwrap_or_default()
}

fn at<'a>(table: &'a Table, path: &[&str]) -> Option<&'a Value> {
    let (last, within) = path.split_last()?;
    within
        .iter()
        .try_fold(table, |table, key| {
            table.get(*key).and_then(Value::as_table)
        })
        .and_then(|table| table.get(*last))
}

fn listened(service: &Table) -> Option<String> {
    service
        .get("listens")
        .and_then(Value::as_integer)
        .map(|port| format!(":{port}"))
}

#[test]
fn the_pinned_core_refuses_nothing_in_the_manifest() {
    let read: Result<lemonfiber_plugin::Manifest, _> = toml::from_str(&read("plugin.toml"));
    let refused: Vec<String> = match read {
        Ok(manifest) => lemonfiber_plugin::refusals(&manifest, &[])
            .into_iter()
            .map(|violation| format!("{}: {}", violation.location, violation.message))
            .collect(),
        Err(why) => vec![why.to_string()],
    };
    assert!(refused.is_empty(), "{refused:#?}");
}

fn adapter(manifest: &Table) -> Table {
    services(manifest)
        .find(|service| service.contains_key("speaks"))
        .cloned()
        .unwrap_or_default()
}

#[test]
fn the_adapter_names_the_image_of_the_traefik_it_fronts() {
    let manifest = manifest();
    let fronted = adapter(&manifest)
        .get("fronts")
        .and_then(Value::as_str)
        .map(|id| service(&manifest, id))
        .unwrap_or_default();
    let named: Vec<String> = traefik_adapter::adapter::about()
        .releases
        .into_iter()
        .map(|release| release.digest)
        .collect();

    assert_eq!(fronted.get("id").and_then(Value::as_str), Some("traefik"));
    assert_eq!(
        fronted.get("digest").and_then(Value::as_str),
        Some(traefik_adapter::adapter::DIGEST)
    );
    assert_eq!(
        fronted.get("tag").and_then(Value::as_str),
        Some(traefik_adapter::adapter::RELEASE)
    );
    assert_eq!(named, vec![traefik_adapter::adapter::DIGEST.to_owned()]);
}

#[test]
fn the_adapter_writes_traefiks_configuration_where_it_shares_it_and_holds_nothing_else() {
    let adapter = adapter(&manifest());

    assert_eq!(
        adapter.get("shares_config").and_then(Value::as_str),
        Some(traefik_adapter::adapter::ADDRESS)
    );
    assert_eq!(
        adapter.get("posture").and_then(Value::as_str),
        Some("confined")
    );
}

#[test]
fn traefik_reads_its_configuration_where_its_starting_files_say_and_writes_none_of_it() {
    let traefik = service(&manifest(), "traefik");

    assert_eq!(
        traefik.get("config_ro").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        traefik.get("config_path").and_then(Value::as_str),
        Some(TRAEFIK_CONFIGURATION)
    );
    assert_eq!(
        at(&starting(), &["providers", "file", "filename"]).and_then(Value::as_str),
        Some(format!("{TRAEFIK_CONFIGURATION}/{DYNAMIC}").as_str())
    );
    assert_eq!(
        at(&starting(), &["providers", "file", "watch"]).and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn traefik_listens_where_the_manifest_publishes_and_sends_plain_http_to_https() {
    let traefik = service(&manifest(), "traefik");
    let published = traefik
        .get("publish")
        .and_then(Value::as_array)
        .and_then(|published| published.first())
        .and_then(Value::as_table)
        .cloned()
        .unwrap_or_default();
    let secure = listened(&published);

    assert_eq!(
        at(&starting(), &["entryPoints", "web", "address"])
            .and_then(Value::as_str)
            .map(str::to_owned),
        listened(&traefik)
    );
    assert_eq!(
        at(&starting(), &["entryPoints", ENTRY_POINT, "address"])
            .and_then(Value::as_str)
            .map(str::to_owned),
        secure
    );
    assert_eq!(published.get("port"), published.get("listens"));
    let redirected = |key: &str| {
        at(
            &starting(),
            &[
                "entryPoints",
                "web",
                "http",
                "redirections",
                "entryPoint",
                key,
            ],
        )
        .cloned()
    };
    assert_eq!(
        redirected("to").and_then(|to| to.as_str().map(str::to_owned)),
        secure
    );
    assert_eq!(redirected("scheme"), Some(Value::from("https")));
    assert_eq!(redirected("permanent"), Some(Value::from(true)));
}

#[test]
fn traefik_serves_no_api_or_dashboard_and_reaches_nothing_of_its_own() {
    let starting = starting();

    for reaching in [
        "api",
        "ping",
        "metrics",
        "tracing",
        "experimental",
        "certificatesResolvers",
    ] {
        assert!(!starting.contains_key(reaching), "{reaching}");
    }
    assert_eq!(
        at(&starting, &["global", "checkNewVersion"]),
        Some(&Value::from(false))
    );
    assert_eq!(
        at(&starting, &["global", "sendAnonymousUsage"]),
        Some(&Value::from(false))
    );
    assert_eq!(
        starting
            .get("providers")
            .and_then(Value::as_table)
            .map(|providers| providers.keys().cloned().collect::<Vec<_>>()),
        Some(vec!["file".to_owned()])
    );
}

#[test]
fn traefik_starts_from_the_dynamic_configuration_the_adapter_writes_for_no_route() {
    let written: serde_json::Value = toml::from_str(&read(&format!("config/traefik/{DYNAMIC}")))
        .unwrap_or(serde_json::Value::Null);

    assert_eq!(
        Ok(written),
        config::rendered(&Routing {
            tls: Tls::Automatic,
            routes: Vec::new(),
        })
    );
}
