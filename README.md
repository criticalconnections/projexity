# Projexity

**Vercel-style deploys, on infrastructure you own.**

Projexity is an open-source deployment platform: connect a GitHub repository,
and it builds and ships your app to *your* servers — any Linux VPS, or your own
Kubernetes cluster — with automatic HTTPS, push-to-deploy, live build logs, and
one-click rollbacks. One-click apps (Supabase, n8n, and friends) included.

> **Status: early alpha.** The scaffold, control plane, and dashboard shell are
> here; the deploy pipeline is landing milestone by milestone (see
> [Roadmap](#roadmap)).

## Why

Vercel's developer experience is superb — until you want your apps on hardware
you control, at prices you control, with no platform lock-in. Projexity keeps
the `git push` → live URL magic and swaps the infrastructure for yours:

- **Bring any server.** A $4 VPS, a Hetzner box, a home lab. Projexity connects
  over SSH, installs Docker and a Caddy reverse proxy with automatic TLS, and
  deploys your apps as containers — zero agents to babysit.
- **Or bring a cluster.** Point it at Kubernetes and the same projects deploy
  as Deployments + Ingress instead.
- **Builds where they run.** Images build on the target server itself: no
  registry required, native CPU architecture, warm layer caches.
- **Self-host in one command.** The control plane is a single Rust binary plus
  Postgres: `docker compose up`.

## Quick start (self-hosting the control plane)

```sh
git clone https://github.com/projexity/projexity
cd projexity
docker compose up -d
```

Then open http://localhost:8080 and create your account.

## Development

Prereqs: Rust (stable), Node 22+, Docker (for Postgres).

```sh
# database
docker compose up -d postgres

# API server (http://localhost:8080)
DATABASE_URL=postgres://projexity:projexity@localhost/projexity cargo run --bin projexity

# dashboard w/ hot reload (http://localhost:5173, proxies /api to :8080)
cd web && npm install && npm run dev
```

Tests and lints:

```sh
cargo fmt --all --check && cargo clippy --workspace --all-targets && cargo test --workspace
cd web && npm run build
```

## Architecture

Cargo workspace + React dashboard:

| Path | What it is |
| --- | --- |
| `crates/core` | Domain types, build/deployment state machines, the `DeployTarget`/`Builder` traits both providers implement. No I/O. |
| `crates/db` | Postgres persistence + migrations + the job queue (`FOR UPDATE SKIP LOCKED`, lease heartbeats). Postgres is the only stateful dependency. |
| `crates/provider-docker` | Docker-on-VPS provider: agentless SSH transport, blue/green deploy choreography, Caddy config rendering, server bootstrap. |
| `crates/provider-k8s` | Kubernetes provider: manifest rendering + server-side apply, rollout watching. |
| `crates/build` | Build pipeline: clone → Dockerfile/Nixpacks plan → remote BuildKit build. |
| `crates/github` | GitHub App integration: tokens, webhooks. |
| `crates/server` | The `projexity` binary: Axum API, SSE log streams, background worker, serves the dashboard. |
| `web/` | React + TypeScript dashboard (Vite, TanStack Router/Query, Tailwind). |

## Roadmap

- [x] **M0** — Scaffold: workspace, control plane skeleton, dashboard shell, `docker compose up`
- [ ] **M1** — Connect a server: SSH onboarding, preflight, Docker + Caddy bootstrap
- [ ] **M2** — Deploy a prebuilt image with a live HTTPS URL
- [ ] **M3** — Build pipeline: Dockerfile & Nixpacks, live build logs, rollbacks
- [ ] **M4** — GitHub App: push-to-deploy
- [ ] **M5** — One-click apps: Supabase, OpenClaw, n8n, Uptime Kuma, Plausible, MinIO…
- [ ] **M6** — Kubernetes provider
- [ ] **M7** — Custom domains UI, docs, launch

## License

Apache-2.0
