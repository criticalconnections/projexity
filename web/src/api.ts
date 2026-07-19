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
  return res.json() as Promise<T>;
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
};
