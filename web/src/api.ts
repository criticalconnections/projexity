/** Minimal typed API client. Replaced by an OpenAPI-generated client once
 * the Rust API grows a utoipa spec. */

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api/v1${path}`, {
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) {
    let message = res.statusText;
    try {
      const body = await res.json();
      if (body.error) message = body.error;
    } catch {
      // non-JSON error body; keep statusText
    }
    throw new ApiError(res.status, message);
  }
  if (res.status === 204) return undefined as T;
  // Some success responses (e.g. 202 Accepted) carry no body.
  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

export interface User {
  id: string;
  email: string;
}

export interface ServerFacts {
  os: string;
  arch: string;
  distro: string | null;
  docker_version: string | null;
  port_80_free: boolean;
  port_443_free: boolean;
  disk_free_bytes: number | null;
  memory_total_bytes: number | null;
  access: "root" | "sudo" | "none";
  caddy_running: boolean;
  marker: string | null;
}

export interface Issue {
  severity: "error" | "warning" | "info";
  message: string;
}

export interface Target {
  id: string;
  name: string;
  kind: "docker_server" | "k8s_cluster";
  status: "pending" | "bootstrapping" | "ready" | "error";
  /** JSON-encoded bootstrap step reports. */
  status_detail: string;
  host: string;
  port: number;
  ssh_user: string;
  public_key: string;
  setup_command: string;
  facts: ServerFacts | null;
  created_at: string;
}

export interface BootstrapStep {
  id: string;
  label: string;
  status: "pending" | "running" | "done" | "skipped" | "failed";
  detail: string;
}

export interface CheckResponse {
  ok: boolean;
  facts: ServerFacts | null;
  issues: Issue[];
  error: string | null;
}

export interface CreateTargetRequest {
  name: string;
  host: string;
  port?: number;
  ssh_user?: string;
}

export type DeploymentStatus =
  | "pending"
  | "deploying"
  | "verifying"
  | "running"
  | "superseded"
  | "stopped"
  | "failed";

export interface ReleaseEnvVar {
  key: string;
  value_enc: string;
}

export interface ReleaseSpec {
  release_id: string;
  app_slug: string;
  image: string;
  container_port: number;
  domains: string[];
  env: ReleaseEnvVar[];
}

export interface Deployment {
  id: string;
  project_id: string;
  kind: "deploy" | "rollback";
  status: DeploymentStatus;
  release_spec: ReleaseSpec;
  provider_ref: { container: string } | null;
  error: string | null;
  created_at: string;
  finished_at: string | null;
}

export interface Project {
  id: string;
  name: string;
  slug: string;
  target_id: string | null;
  image: string | null;
  repo: string | null;
  branch: string;
  container_port: number;
  domains: string[];
  latest_deployment: Deployment | null;
  created_at: string;
}

export interface EnvVar {
  key: string;
  value: string;
  is_build_time: boolean;
}

export interface CreateProjectRequest {
  name: string;
  target_id: string;
  image?: string;
  repo?: string;
  branch?: string;
  container_port: number;
}

export interface GithubInstallation {
  installation_id: number;
  account_login: string;
}

export interface GithubAppStatus {
  configured: boolean;
  app_slug: string | null;
  app_url: string | null;
  installations: GithubInstallation[];
  webhook_url: string;
  public_url_is_local: boolean;
}

export interface GithubManifest {
  action_url: string;
  manifest: Record<string, unknown>;
}

export interface GithubRepo {
  full_name: string;
  private: boolean;
  default_branch: string;
}

export interface GithubReposResponse {
  connected: boolean;
  repos: GithubRepo[];
}

export interface CatalogEnvField {
  key: string;
  /** Always null in catalog responses. */
  generate: string | null;
  label: string | null;
  default: string | null;
  required: boolean;
}

export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  icon: string;
  website: string;
  env: CatalogEnvField[];
}

export interface AppInstall {
  id: string;
  target_id: string;
  template_id: string;
  name: string;
  slug: string;
  status: "installing" | "running" | "error" | "removing";
  /** JSON-encoded install step reports (same shape as bootstrap steps). */
  status_detail: string;
  /** service name -> hostname */
  domains: Record<string, string>;
  created_at: string;
}

export interface InstallAppRequest {
  template_id: string;
  target_id: string;
  name?: string;
  env: Record<string, string>;
}

/** A deployment that is still in flight (not yet in a terminal-ish state). */
export function isDeploymentActive(d: Deployment | null | undefined): boolean {
  return (
    d?.status === "pending" ||
    d?.status === "deploying" ||
    d?.status === "verifying"
  );
}

/** SSE endpoint for a deployment's build/deploy log. Use with EventSource. */
export function deploymentLogsUrl(deploymentId: string): string {
  return `/api/v1/deployments/${deploymentId}/logs/stream`;
}

/** SSE endpoint for a project's live container logs. Use with EventSource. */
export function runtimeLogsUrl(projectId: string): string {
  return `/api/v1/projects/${projectId}/runtime-logs/stream`;
}

export function parseBootstrapSteps(detail: string): BootstrapStep[] {
  try {
    const parsed = JSON.parse(detail);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export const api = {
  me: () => request<User>("/auth/me"),
  login: (email: string, password: string) =>
    request<User>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),
  register: (email: string, password: string) =>
    request<User>("/auth/register", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),
  logout: () => request<void>("/auth/logout", { method: "POST" }),

  listTargets: () => request<Target[]>("/targets"),
  createTarget: (body: CreateTargetRequest) =>
    request<Target>("/targets", { method: "POST", body: JSON.stringify(body) }),
  getTarget: (id: string) => request<Target>(`/targets/${id}`),
  deleteTarget: (id: string) =>
    request<void>(`/targets/${id}`, { method: "DELETE" }),
  checkTarget: (id: string) =>
    request<CheckResponse>(`/targets/${id}/check`, { method: "POST" }),
  bootstrapTarget: (id: string) =>
    request<Target>(`/targets/${id}/bootstrap`, { method: "POST" }),

  listProjects: () => request<Project[]>("/projects"),
  createProject: (body: CreateProjectRequest) =>
    request<Project>("/projects", { method: "POST", body: JSON.stringify(body) }),
  getProject: (id: string) => request<Project>(`/projects/${id}`),
  deleteProject: (id: string) =>
    request<void>(`/projects/${id}`, { method: "DELETE" }),
  getProjectEnv: (id: string) => request<EnvVar[]>(`/projects/${id}/env`),
  putProjectEnv: (id: string, vars: EnvVar[]) =>
    request<void>(`/projects/${id}/env`, {
      method: "PUT",
      body: JSON.stringify(vars),
    }),
  deployProject: (id: string) =>
    request<Deployment>(`/projects/${id}/deploy`, { method: "POST" }),
  listProjectDeployments: (id: string) =>
    request<Deployment[]>(`/projects/${id}/deployments`),
  listDeployments: () => request<Deployment[]>("/deployments"),
  getDeployment: (id: string) => request<Deployment>(`/deployments/${id}`),
  rollbackDeployment: (id: string) =>
    request<Deployment>(`/deployments/${id}/rollback`, { method: "POST" }),

  listTemplates: () => request<CatalogEntry[]>("/templates"),
  listApps: () => request<AppInstall[]>("/apps"),
  installApp: (body: InstallAppRequest) =>
    request<AppInstall>("/apps", { method: "POST", body: JSON.stringify(body) }),
  getApp: (id: string) => request<AppInstall>(`/apps/${id}`),
  uninstallApp: (id: string, purge = false) =>
    request<void>(`/apps/${id}${purge ? "?purge=true" : ""}`, {
      method: "DELETE",
    }),

  githubStatus: () => request<GithubAppStatus>("/github/app"),
  githubManifest: () => request<GithubManifest>("/github/manifest"),
  githubRepos: () => request<GithubReposResponse>("/github/repos"),
};
