import type { DeploymentStatus } from "../api";

const STYLES: Record<DeploymentStatus, string> = {
  running: "bg-emerald-500/10 text-emerald-400",
  pending: "bg-amber-500/10 text-amber-400 animate-pulse",
  deploying: "bg-amber-500/10 text-amber-400 animate-pulse",
  verifying: "bg-amber-500/10 text-amber-400 animate-pulse",
  failed: "bg-red-500/10 text-red-400",
  superseded: "bg-zinc-500/10 text-zinc-400",
  stopped: "bg-zinc-500/10 text-zinc-400",
};

const LABELS: Record<DeploymentStatus, string> = {
  running: "running",
  pending: "queued",
  deploying: "deploying…",
  verifying: "verifying…",
  failed: "failed",
  superseded: "superseded",
  stopped: "stopped",
};

/** Small colored status pill for a deployment. `null` renders as a muted
 * "never deployed" pill (for projects with no deployments yet). */
export function StatusPill({ status }: { status: DeploymentStatus | null | undefined }) {
  const cls = status ? STYLES[status] : "bg-zinc-500/10 text-zinc-500";
  const label = status ? LABELS[status] : "never deployed";
  return (
    <span
      className={`inline-flex items-center whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-medium ${cls}`}
    >
      {label}
    </span>
  );
}
