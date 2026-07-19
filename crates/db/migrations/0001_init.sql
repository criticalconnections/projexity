-- Projexity initial schema.
-- Conventions: UUIDv7 ids generated in the application; TEXT enums checked in
-- the app's state machines (projexity-core) and constrained here.

CREATE TABLE users (
    id            UUID PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Hand-rolled session store: opaque random token in an HttpOnly cookie.
CREATE TABLE sessions (
    token      TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);

-- A place to deploy: one VPS (docker_server) or one cluster (k8s_cluster).
-- Credentials (SSH private key / kubeconfig) live envelope-encrypted inside
-- config; never store them in plaintext columns.
CREATE TABLE targets (
    id         UUID PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('docker_server', 'k8s_cluster')),
    status     TEXT NOT NULL DEFAULT 'pending'
               CHECK (status IN ('pending', 'bootstrapping', 'ready', 'error')),
    status_detail TEXT NOT NULL DEFAULT '',
    config     JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE projects (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_id       UUID REFERENCES targets(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL UNIQUE,
    repo_owner      TEXT,
    repo_name       TEXT,
    branch          TEXT NOT NULL DEFAULT 'main',
    root_dir        TEXT NOT NULL DEFAULT '.',
    dockerfile_path TEXT,
    container_port  INTEGER NOT NULL DEFAULT 3000,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE env_vars (
    id               UUID PRIMARY KEY,
    project_id       UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    key              TEXT NOT NULL,
    value_ciphertext BYTEA NOT NULL,
    is_build_time    BOOLEAN NOT NULL DEFAULT false,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, key)
);

CREATE TABLE domains (
    id           UUID PRIMARY KEY,
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    hostname     TEXT NOT NULL UNIQUE,
    is_generated BOOLEAN NOT NULL DEFAULT false,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE builds (
    id             UUID PRIMARY KEY,
    project_id     UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status         TEXT NOT NULL DEFAULT 'queued'
                   CHECK (status IN ('queued', 'cloning', 'building',
                                     'succeeded', 'failed', 'canceled', 'superseded')),
    commit_sha     TEXT,
    commit_message TEXT,
    image_ref      TEXT,
    error          TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at     TIMESTAMPTZ,
    finished_at    TIMESTAMPTZ
);
CREATE INDEX builds_project_idx ON builds (project_id, created_at DESC);

CREATE TABLE deployments (
    id           UUID PRIMARY KEY,
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    build_id     UUID REFERENCES builds(id) ON DELETE SET NULL,
    -- 'deploy' | 'rollback'; a rollback is a NEW deployment pointing at an
    -- older release spec, never a state on the failed one.
    kind         TEXT NOT NULL DEFAULT 'deploy' CHECK (kind IN ('deploy', 'rollback')),
    status       TEXT NOT NULL DEFAULT 'pending'
                 CHECK (status IN ('pending', 'deploying', 'verifying', 'running',
                                   'superseded', 'stopped', 'failed')),
    -- Immutable snapshot: pinned image digest + env by reference. Replaying
    -- this spec reproduces the release exactly (that's what rollback does).
    release_spec JSONB NOT NULL,
    provider_ref JSONB,
    error        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at  TIMESTAMPTZ
);
CREATE INDEX deployments_project_idx ON deployments (project_id, created_at DESC);

-- Double-enqueue is structurally impossible: at most one in-flight deployment
-- per project.
CREATE UNIQUE INDEX deployments_one_active_per_project
    ON deployments (project_id)
    WHERE status IN ('pending', 'deploying', 'verifying');

CREATE TABLE deployment_logs (
    deployment_id UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    seq           BIGINT NOT NULL,
    stream        TEXT NOT NULL CHECK (stream IN ('build', 'runtime', 'deploy')),
    ts            TIMESTAMPTZ NOT NULL DEFAULT now(),
    text          TEXT NOT NULL,
    PRIMARY KEY (deployment_id, seq)
);

-- Postgres-backed job queue: FOR UPDATE SKIP LOCKED claims + lease heartbeat.
CREATE TABLE jobs (
    id               UUID PRIMARY KEY,
    kind             TEXT NOT NULL,
    payload          JSONB NOT NULL DEFAULT '{}',
    status           TEXT NOT NULL DEFAULT 'queued'
                     CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    attempts         INTEGER NOT NULL DEFAULT 0,
    max_attempts     INTEGER NOT NULL DEFAULT 3,
    run_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_expires_at TIMESTAMPTZ,
    last_error       TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX jobs_claim_idx ON jobs (status, run_at);
