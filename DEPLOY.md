# Deploying the Projexity control plane

This takes the control plane live at your own domain with automatic HTTPS —
e.g. `https://projexity.io`. Any small Linux VPS works (1 vCPU / 1 GB is
plenty; builds run on your *target* servers, not here).

## 1. DNS

Create an `A` record for your domain (e.g. `projexity.io`) pointing at the
VPS's IP. (An `AAAA` record too, if the box has IPv6.)

## 2. On the VPS

```sh
# install docker if missing
curl -fsSL https://get.docker.com | sh

git clone https://github.com/criticalconnections/projexity
cd projexity

cat > .env <<EOF
PJX_DOMAIN=projexity.io
POSTGRES_PASSWORD=$(openssl rand -hex 24)
PJX_MASTER_KEY=$(openssl rand -hex 32)
EOF
chmod 600 .env

docker compose -f docker-compose.prod.yml up -d --build
```

That's it. Caddy obtains certificates on first request; open
`https://your-domain`, create your account, and connect a server.

> **Back up `.env` (especially `PJX_MASTER_KEY`) and the `pgdata` volume.**
> The master key encrypts stored credentials — losing it means reconnecting
> every server and re-entering every secret.

## 3. GitHub App (push-to-deploy)

With a public URL this is fully click-through: **Settings → Create GitHub
App** → approve on GitHub → install on your account. Webhooks now reach
`https://your-domain/api/v1/webhooks/github` and pushes deploy automatically.

## Upgrading

```sh
git pull && docker compose -f docker-compose.prod.yml up -d --build
```

Migrations run automatically on boot.

## Notes

- The first registered account is yours; registration is currently open, so
  put the instance behind your own judgment (multi-user/teams are on the
  roadmap pre-1.0).
- The control-plane VPS and your deploy-target servers are separate roles.
  Using the same box for both is possible but the platform's app proxy and
  the control plane's Caddy would contend for ports 80/443 — keep them
  separate for now.
- Local development still uses plain `docker-compose.yml` (HTTP on :8080).
