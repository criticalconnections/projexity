-- M4: GitHub App integration. Each self-hosted instance registers its OWN
-- GitHub App (via the manifest flow), so credentials live here — encrypted
-- with the instance master key, like all stored secrets.

CREATE TABLE github_apps (
    id                 UUID PRIMARY KEY,
    user_id            UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    app_id             BIGINT NOT NULL,
    slug               TEXT NOT NULL,
    html_url           TEXT NOT NULL,
    client_id          TEXT NOT NULL DEFAULT '',
    pem_enc            TEXT NOT NULL,
    webhook_secret_enc TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE github_installations (
    id              UUID PRIMARY KEY,
    installation_id BIGINT NOT NULL UNIQUE,
    account_login   TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
