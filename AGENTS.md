# AGENTS.md — plugin-traefik

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

Traefik as a first-party lemonfiber plugin: `plugin.toml`, its starting files
under `config/traefik/`, and the adapter that speaks the `proxy.front@1`
contract in front of Traefik, the second proxy, which stands in for Caddy where
`F14-R31` proves `proxy.front` by a substitute. The adapter is the Rust crate at
the root, built on `lemonfiber-adapter-kit` from the core at the commit
`Cargo.toml` pins, into the image the `Dockerfile` builds. `release.yml`
publishes that image from every commit on `main` and from the train's tag.

`conformance/` holds one recording per case of `proxy.front@1`.
`lemonfiber plugin record .`, at the commit `targets.toml` names, writes them
against a throwaway Traefik at the digest `plugin.toml` pins and the adapter it
pins; CI takes every recording again with the adapter built from this tree.
Re-record whenever the adapter's answers or either digest change.
`tests/conformance.rs` reads every recording with the core's own client.

## The rules you cannot break

- **The plugin id is the service's id** (`ARCH-R266`). The container, the
  configuration directory, the settings and the journal keep their names.
- **Two services: `traefik` and `traefik-adapter`.** `traefik-adapter` fronts
  Traefik, speaks `proxy.front@1`, provides `proxy.front` and shares Traefik's
  configuration directory at `/traefik` (`ARCH-R339`), under the confined
  posture.
- **There is no `replaces`.** Traefik is a substitute, offered beside Caddy,
  not a move in place, so it holds host ports of its own: 8080 for plain HTTP
  and 8443 for HTTPS, which Traefik listens on too.
- **Traefik runs its upstream's image as the operator's uid** (`user_mode =
  "uid"`), under the standard posture, as Caddy's rebuilt image does. The proof
  `traefik.holds-no-privilege` holds it to running as a user that is not root,
  with no capability it holds, may take up or passes on, and no-new-privileges
  set.
- **Its proofs' recordings sit under `recordings/traefik/`**, and its starting
  files under `config/traefik/` (`ARCH-R271`).
- **Every service declares `reaches` and `asks_for`** (`ARCH-R270`).
- **The reader is spec's `first-party-plugin` workflow's,** and lemonfiber
  records the recordings itself. There is no Python here.
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **Traefik reads `traefik.toml` once and watches `dynamic.toml`.** Its
  configuration directory is mounted read-only (`config_ro`). `traefik.toml`
  turns the API, the dashboard, the update check and usage data off and names
  the file provider as the only provider. The adapter writes `dynamic.toml`
  and the table it came from, `routing.json`, through
  `lemonfiber_adapter_kit::shared`, and `apply` and `reload` answer that the
  table is in force at once. A configuration that is not the one rendered from
  the table is refused, not served, and `reload` writes it again from the
  table. Traefik refuses an empty router table, so a table with no route is
  written as no `http` section at all.
- **Every value is held to its grammar before anything is written**: a name is
  a DNS name, never a wildcard and never the name of two routes; a host is a DNS
  name or an IPv4 address; a port is not zero; a certificate or key is a plain
  file beneath the configuration directory and never Traefik's or the
  adapter's own files.
- **The core's crates move with the train.** The `rev` in `Cargo.toml` is the
  core commit `targets.toml` names, the same for every `lemonfiber-*` crate.
- **Moving Traefik's digest** means recording every proof and conformance case
  again against the new image in the same change, moving `RELEASE` and `DIGEST`
  in `src/adapter.rs` with it, and committing the `proofs.json` the checks
  write; `tests/manifest.rs` fails until they name the image the manifest's
  `traefik` runs.

## Checks

```
just ci
```

The gates that need nothing but this tree. The plugin verdict and the
recordings run in CI.
