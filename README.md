# Traefik for lemonfiber

**Friendly hostnames instead of ports.**
Traefik is a reverse proxy, offered as the proxy in Caddy's place.
Without it: Use localhost:8989 and friends.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) offers
this repository, pinned by a revision and the digests of its manifest and its
tree, and installs it where the operator asks for it. Traefik runs from its own
image, `docker.io/library/traefik`, pinned by tag and digest, as the operator's
user. It answers plain HTTP on 8080 with a redirect to HTTPS on 8443.

This repository also holds the adapter that speaks lemonfiber's `proxy.front`
contract in front of Traefik: a Rust crate built on the core's adapter kit, into
the image `ghcr.io/lemonfiber/traefik-adapter`, published from every commit on
`main` and from each tag the release train cuts. It writes the routes and
certificates the core hands it as `dynamic.toml` into Traefik's configuration
directory, which it shares and Traefik only reads. Traefik's file provider
watches that file and serves what it says at once, with no restart. Traefik's
API and dashboard are off, and it checks for no new release and sends no usage
data.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The plugin's manifest |
| `config/traefik/` | The files Traefik starts with, written into its configuration directory |
| `src/` | The adapter |
| `recordings/` | The recordings of the manifest's proofs |
| `Dockerfile` | The adapter's image |
| `targets.toml` | The lemonfiber its checks run against, and the core the adapter is built on |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
proof against its recording, and every pinned digest. The adapter is held to its
format, its lints, its tests, full line coverage and its dependency policy, and
its image runs as lemonfiber runs it. This repository holds no Python of its own.
`just ci` runs the gates that need nothing but this tree.

## Licence

The code and data in this repository are under the licence in `LICENSE`.
Traefik itself is under MIT and is not distributed here: this repository names
an image, it does not contain one.
