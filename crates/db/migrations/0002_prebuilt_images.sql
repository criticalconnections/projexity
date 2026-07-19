-- M2: projects can deploy a prebuilt image (no repo) — `image` is set for
-- image-based projects, repo_owner/repo_name for git-based ones (M3+).
ALTER TABLE projects ADD COLUMN image TEXT;
