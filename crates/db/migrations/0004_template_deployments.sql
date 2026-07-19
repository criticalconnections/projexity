-- M5: one-click app catalog. A template deployment is a docker-compose stack
-- installed on a target from a curated template.

CREATE TABLE template_deployments (
    id            UUID PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_id     UUID NOT NULL REFERENCES targets(id) ON DELETE CASCADE,
    template_id   TEXT NOT NULL,
    name          TEXT NOT NULL,
    slug          TEXT NOT NULL UNIQUE,
    status        TEXT NOT NULL DEFAULT 'installing'
                  CHECK (status IN ('installing', 'running', 'error', 'removing')),
    status_detail TEXT NOT NULL DEFAULT '',
    -- Rendered env values (incl. generated secrets), encrypted; replaying
    -- them makes upgrades/repairs deterministic.
    env_enc       TEXT NOT NULL DEFAULT '',
    -- service name -> hostname
    domains       JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX template_deployments_user_idx ON template_deployments (user_id, created_at DESC);
