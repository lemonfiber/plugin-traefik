use lemonfiber_ports::service::{Route, Routing, Tls};
use serde_json::json;

use super::{rendered, Refused};

fn route(name: &str, host: &str, port: u16) -> Route {
    Route {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
    }
}

fn table(tls: Tls) -> Routing {
    Routing {
        tls,
        routes: vec![
            route("watch.home.example", "door", 8096),
            route("requests.home.example", "seerr", 5055),
        ],
    }
}

fn files(certificate: &str, key: &str) -> Tls {
    Tls::Files {
        certificate: certificate.to_owned(),
        key: key.to_owned(),
    }
}

const INJECTIONS: [&str; 20] = [
    "",
    "a b",
    "a\nb",
    "a\r\nb",
    "a\tb",
    "a}\n:80 {\n\tfile_server",
    "a { admin localhost:2019 }",
    "{$DOMAIN}",
    "{env.HOME}",
    "home.{$DOMAIN:home.local}",
    "import /etc/passwd",
    "# a comment",
    "a\"b",
    "a'b",
    "a;b",
    "a\u{0}b",
    "a`b",
    "a`) || PathPrefix(`/",
    "a@internal",
    "a=b",
];

#[test]
fn every_route_is_a_router_on_its_name_over_https_handing_to_its_own_service() {
    let config = rendered(&table(Tls::Automatic));
    assert!(config.is_ok());
    let config = config.unwrap_or_default();

    assert_eq!(
        config.pointer("/http/routers/route-0"),
        Some(&json!({
            "rule": "Host(`watch.home.example`)",
            "entryPoints": ["websecure"],
            "service": "route-0",
            "tls": {},
        }))
    );
    assert_eq!(
        config.pointer("/http/services/route-1/loadBalancer/servers"),
        Some(&json!([{ "url": "http://seerr:5055" }]))
    );
    assert_eq!(config.pointer("/tls"), None);
}

#[test]
fn no_route_is_a_configuration_with_no_http_section_which_traefik_would_refuse_empty() {
    let config = rendered(&Routing {
        tls: Tls::Automatic,
        routes: Vec::new(),
    });

    assert_eq!(config, Ok(json!({})));
}

#[test]
fn certificates_are_served_from_files_beneath_the_configuration_for_every_name() {
    let config = rendered(&table(files(
        "certificates/house.crt",
        "certificates/house.key",
    )))
    .unwrap_or_default();

    assert_eq!(
        config.pointer("/tls/stores/default/defaultCertificate"),
        Some(&json!({
            "certFile": "/etc/traefik/certificates/house.crt",
            "keyFile": "/etc/traefik/certificates/house.key",
        }))
    );
}

#[test]
fn a_host_may_be_an_ipv4_address() {
    let routing = Routing {
        tls: Tls::Automatic,
        routes: vec![route("watch.home.example", "192.0.2.7", 8096)],
    };

    assert!(rendered(&routing).is_ok());
}

#[test]
fn a_name_two_routes_answer_on_is_refused_rather_than_the_last_taking_it() {
    for second in ["watch.home.example", "WATCH.home.example"] {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![
                route("watch.home.example", "door", 8096),
                route(second, "elsewhere", 80),
            ],
        };
        assert_eq!(rendered(&routing), Err(Refused::Twice(second.to_owned())));
    }
}

#[test]
fn two_names_may_reach_one_service() {
    let routing = Routing {
        tls: Tls::Automatic,
        routes: vec![
            route("watch.home.example", "door", 8096),
            route("tv.home.example", "door", 8096),
        ],
    };

    assert!(rendered(&routing).is_ok());
}

#[test]
fn a_wildcard_that_would_answer_for_another_routes_name_is_refused() {
    for wildcard in ["*.home.example", "*.example", "*"] {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![
                route(wildcard, "elsewhere", 80),
                route("watch.home.example", "door", 8096),
            ],
        };
        assert_eq!(
            rendered(&routing),
            Err(Refused::Unnamed(wildcard.to_owned()))
        );
    }
}

#[test]
fn every_route_answers_on_its_own_name_alone() {
    let config = rendered(&table(Tls::Automatic)).unwrap_or_default();
    let rules: Vec<serde_json::Value> = config
        .pointer("/http/routers")
        .and_then(serde_json::Value::as_object)
        .map(|routers| {
            routers
                .values()
                .filter_map(|router| router.get("rule").cloned())
                .collect()
        })
        .unwrap_or_default();

    assert_eq!(
        rules,
        [
            json!("Host(`watch.home.example`)"),
            json!("Host(`requests.home.example`)")
        ]
    );
}

#[test]
fn a_name_carrying_anything_but_a_dns_name_is_refused() {
    let mut names: Vec<String> = INJECTIONS.iter().map(|one| (*one).to_owned()).collect();
    names.extend(
        [
            ".lead",
            "trail.",
            "-a",
            "a-",
            "a..b",
            "*",
            "a.*.b",
            "**.a",
            "1.2.3.4:80",
        ]
        .map(str::to_owned),
    );
    names.push("a".repeat(64));
    names.push(format!("{}.a", "a.".repeat(127)));
    for name in names {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![route(&name, "door", 8096)],
        };
        assert_eq!(rendered(&routing), Err(Refused::Unnamed(name)));
    }
}

#[test]
fn a_host_carrying_anything_but_a_dns_name_or_an_ipv4_address_is_refused() {
    let mut hosts: Vec<String> = INJECTIONS.iter().map(|one| (*one).to_owned()).collect();
    hosts.extend(["*.door", "door:80", "[::1]", "::1", "256.0.0.1", "door/x"].map(str::to_owned));
    for host in hosts {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![route("watch.home.example", &host, 8096)],
        };
        assert_eq!(rendered(&routing), Err(Refused::Unhosted(host)));
    }
}

#[test]
fn port_zero_is_refused() {
    let routing = Routing {
        tls: Tls::Automatic,
        routes: vec![route("watch.home.example", "door", 0)],
    };

    assert_eq!(rendered(&routing), Err(Refused::Unported(0)));
}

#[test]
fn a_certificate_that_is_not_a_plain_file_beneath_the_configuration_is_refused() {
    let mut paths: Vec<String> = INJECTIONS.iter().map(|one| (*one).to_owned()).collect();
    paths.extend(
        [
            "/etc/passwd",
            "../key",
            "certificates//key",
            "./key",
            ".hidden",
            "a/../../b",
            "traefik.toml",
            "dynamic.toml",
            "routing.json",
            "a/b/c/d/e",
            "a{b}",
            "a$b",
        ]
        .map(str::to_owned),
    );
    paths.push("a".repeat(65));
    for path in paths {
        assert_eq!(
            rendered(&table(files(&path, "house.key"))),
            Err(Refused::Outside(path.clone()))
        );
        assert_eq!(
            rendered(&table(files("house.crt", &path))),
            Err(Refused::Outside(path))
        );
    }
}

#[test]
fn whatever_is_accepted_holds_no_character_a_rule_or_matcher_needs() {
    let alphabet: Vec<char> = "ab-.*`(|$ \n#\"/:_".chars().collect();
    let mut accepted = 0;
    for seed in 0..20_000_u32 {
        let mut value = String::new();
        let mut at = seed;
        for _ in 0..6 {
            let index = usize::try_from(at % 16).unwrap_or_default();
            value.push(alphabet.get(index).copied().unwrap_or('a'));
            at /= 16;
        }
        let routing = Routing {
            tls: files(&value, "house.key"),
            routes: vec![route(&value, &value, 8096)],
        };
        if rendered(&routing).is_ok() {
            accepted += 1;
            assert!(!value.contains(['`', '(', '|', '$', ' ', '\n', '#', '"', ':', '/']));
        }
    }
    assert!(accepted > 0);
}
