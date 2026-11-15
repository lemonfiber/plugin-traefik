use std::path::{Path, PathBuf};

use lemonfiber_ports::service::{Applied, Failure, Proxying, Route, Routing, Tls};

use super::config::{self, DYNAMIC, TABLE};
use super::Traefik;

struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("traefik-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }

    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap_or_default()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn table() -> Routing {
    Routing {
        tls: Tls::Automatic,
        routes: vec![Route {
            name: "watch.home.example".to_owned(),
            host: "door".to_owned(),
            port: 8096,
        }],
    }
}

fn traefik(directory: &Path) -> Traefik {
    Traefik::at(directory)
}

fn configuration(directory: &Directory) -> Option<serde_json::Value> {
    toml::from_str(&directory.file(DYNAMIC)).ok()
}

#[tokio::test]
async fn a_table_applied_is_written_as_traefiks_dynamic_configuration_in_force_at_once() {
    let directory = Directory::new("applied");
    let traefik = traefik(&directory.0);

    assert!(matches!(traefik.apply(&table()).await, Ok(Applied::Now)));

    assert_eq!(configuration(&directory), config::rendered(&table()).ok());
    assert_eq!(traefik.routing().await.ok(), Some(table()));
    assert!(!directory.file(TABLE).is_empty());
}

#[tokio::test]
async fn nothing_written_yet_reads_as_no_route() {
    let directory = Directory::new("unwritten");
    let _ = std::fs::write(
        directory.0.join(DYNAMIC),
        "[http.routers.theirs]\nrule = \"PathPrefix(`/`)\"\n",
    );

    assert_eq!(
        traefik(&directory.0).routing().await.ok(),
        Some(Routing {
            tls: Tls::Automatic,
            routes: Vec::new(),
        })
    );
}

#[tokio::test]
async fn a_configuration_changed_by_hand_is_refused_until_a_reload_writes_it_again() {
    let directory = Directory::new("edited");
    let traefik = traefik(&directory.0);
    assert!(traefik.apply(&table()).await.is_ok());
    let _ = std::fs::write(
        directory.0.join(DYNAMIC),
        "[http.routers.theirs]\nrule = \"PathPrefix(`/`)\"\nservice = \"api@internal\"\n",
    );

    assert!(matches!(
        traefik.routing().await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(traefik.reload().await, Ok(Applied::Now)));
    assert_eq!(configuration(&directory), config::rendered(&table()).ok());
    assert_eq!(traefik.routing().await.ok(), Some(table()));
}

#[tokio::test]
async fn a_reload_writes_the_same_configuration_again() {
    let directory = Directory::new("reloaded");
    let traefik = traefik(&directory.0);
    assert!(traefik.apply(&table()).await.is_ok());
    let before = directory.file(DYNAMIC);

    assert!(matches!(traefik.reload().await, Ok(Applied::Now)));

    assert_eq!(directory.file(DYNAMIC), before);
}

#[tokio::test]
async fn a_reload_with_no_table_written_writes_nothing() {
    let directory = Directory::new("reloaded-unwritten");

    assert!(matches!(
        traefik(&directory.0).reload().await,
        Ok(Applied::Now)
    ));

    assert!(directory.file(DYNAMIC).is_empty());
}

#[tokio::test]
async fn a_reload_of_a_table_no_configuration_could_carry_writes_nothing() {
    let directory = Directory::new("reloaded-unnamed");
    let mut unnamed = table();
    unnamed.routes = vec![Route {
        name: "*.home.example".to_owned(),
        host: "door".to_owned(),
        port: 8096,
    }];
    let _ = std::fs::write(
        directory.0.join(TABLE),
        serde_json::to_vec(&unnamed).unwrap_or_default(),
    );

    assert!(matches!(
        traefik(&directory.0).reload().await,
        Err(Failure::Refused { .. })
    ));
    assert!(directory.file(DYNAMIC).is_empty());
}

#[tokio::test]
async fn a_route_carrying_a_directive_is_refused_before_anything_is_written() {
    let directory = Directory::new("unnamed");
    let traefik = traefik(&directory.0);
    let mut unnamed = table();
    unnamed.routes = vec![Route {
        name: "a`) || PathPrefix(`/".to_owned(),
        host: "door".to_owned(),
        port: 8096,
    }];

    assert!(matches!(
        traefik.apply(&unnamed).await,
        Err(Failure::Refused { .. })
    ));
    assert!(directory.file(DYNAMIC).is_empty());
    assert!(directory.file(TABLE).is_empty());
}

#[tokio::test]
async fn a_directory_that_is_not_there_is_unavailable() {
    let traefik = traefik(Path::new("/nonexistent/traefik-configuration"));

    assert!(!traefik.answers().await);
    assert!(matches!(
        traefik.apply(&table()).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_table_that_cannot_be_read_is_refused() {
    let directory = Directory::new("unreadable");
    let _ = std::fs::write(directory.0.join(TABLE), "not json");

    assert!(matches!(
        traefik(&directory.0).routing().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_link_planted_at_the_configuration_is_replaced_and_never_written_through() {
    let directory = Directory::new("linked-configuration");
    let outside = Directory::new("linked-configuration-outside");
    let _ = std::fs::write(outside.0.join(DYNAMIC), "theirs");
    let _ = std::os::unix::fs::symlink(outside.0.join(DYNAMIC), directory.0.join(DYNAMIC));

    assert!(traefik(&directory.0).apply(&table()).await.is_ok());

    assert_eq!(outside.file(DYNAMIC), "theirs");
    let held = std::fs::symlink_metadata(directory.0.join(DYNAMIC));
    assert!(held.is_ok_and(|held| held.file_type().is_file()));
}

#[tokio::test]
async fn a_table_that_is_a_link_or_not_a_plain_file_is_refused_naming_it() {
    let directory = Directory::new("linked-table");
    let outside = Directory::new("linked-table-outside");
    let _ = std::fs::write(outside.0.join(TABLE), "{}");
    let _ = std::os::unix::fs::symlink(outside.0.join(TABLE), directory.0.join(TABLE));
    let unplain = Directory::new("unplain-table");
    let _ = std::fs::create_dir_all(unplain.0.join(TABLE));

    for at in [&directory, &unplain] {
        let refused = traefik(&at.0).routing().await;
        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains(TABLE)),
            "{refused:?}"
        );
    }
}

#[tokio::test]
async fn a_table_with_no_configuration_beside_it_is_refused() {
    let directory = Directory::new("unconfigured");
    let traefik = traefik(&directory.0);
    assert!(traefik.apply(&table()).await.is_ok());
    let _ = std::fs::remove_file(directory.0.join(DYNAMIC));

    assert!(matches!(
        traefik.routing().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn every_refusal_of_a_table_says_what_it_refused() {
    let directory = Directory::new("refusals");
    let traefik = traefik(&directory.0);
    let route = |name: &str, host: &str, port: u16| Route {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
    };
    let mut twice = table();
    twice.routes.push(route("WATCH.home.example", "door", 8097));
    let mut unhosted = table();
    unhosted.routes = vec![route("watch.home.example", "door\n", 8096)];
    let mut unported = table();
    unported.routes = vec![route("watch.home.example", "door", 0)];
    let mut outside = table();
    outside.tls = Tls::Files {
        certificate: "../../etc/shadow".to_owned(),
        key: "key.pem".to_owned(),
    };

    for (asked, said) in [
        (twice, "the name of two routes"),
        (unhosted, "not a DNS name or an IPv4 address"),
        (unported, "not a port a service answers on"),
        (outside, "not a plain file beneath"),
    ] {
        let refused = traefik.apply(&asked).await;
        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains(said)),
            "{refused:?}"
        );
    }
    assert!(directory.file(DYNAMIC).is_empty());
}

#[tokio::test]
async fn a_table_written_by_hand_that_no_configuration_could_carry_is_refused() {
    let directory = Directory::new("handwritten");
    let mut unnamed = table();
    unnamed.routes = vec![Route {
        name: "*.home.example".to_owned(),
        host: "door".to_owned(),
        port: 8096,
    }];
    let _ = std::fs::write(
        directory.0.join(TABLE),
        serde_json::to_vec(&unnamed).unwrap_or_default(),
    );

    let refused = traefik(&directory.0).routing().await;

    assert!(
        matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains("not a plain DNS name")),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_configuration_that_is_not_toml_is_refused() {
    let directory = Directory::new("untoml");
    let traefik = traefik(&directory.0);
    assert!(traefik.apply(&table()).await.is_ok());
    let _ = std::fs::write(directory.0.join(DYNAMIC), "[http");

    let refused = traefik.routing().await;

    assert!(
        matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains("could not be read")),
        "{refused:?}"
    );
}
