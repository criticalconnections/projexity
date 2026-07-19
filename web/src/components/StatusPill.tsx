import type { DeploymentStatus } from "../api";

interface PillStyle {
  pill: string;
  dot: string;
  /** Pulse the dot while the deployment is in flight. */
  active: boolean;
}

const STYLES: Record<DeploymentStatus, PillStyle> = {
  running: {
    pill: "border-emerald-500/25 bg-emerald-500/10 text-emerald-300",
    dot: "bg-emerald-400",
    active: false,
  },
  pending: {
    pill: "border-amber-500/25 bg-amber-500/10 text-amber-300",
    dot: "bg-amber-400",
    active: true,
  },
  deploying: {
    pill: "border-amber-500/25 bg-amber-500/10 text-amber-300",
    dot: "bg-amber-400",
    active: true,
  },
  verifying: {
    pill: "border-amber-500/25 bg-amber-500/10 text-amber-300",
    dot: "bg-amber-400",
    active: true,
  },
  failed: {
    pill: "border-red-500/25 bg-red-500/10 text-red-300",
    dot: "bg-red-400",
    active: false,
  },
  superseded: {
    pill: "border-white/10 bg-white/[0.04] text-zinc-400",
    dot: "bg-zinc-500",
    active: false,
  },
  stopped: {
    pill: "border-white/10 bg-white/[0.04] text-zinc-400",
    dot: "bg-zinc-500",
    active: false,
  },
};

const NONE: PillStyle = {
  pill: "border-white/10 bg-white/[0.03] text-zinc-500",
  dot: "bg-zinc-600",
  active: false,
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
  const s = status ? STYLES[status] : NONE;
  const label = status ? LABELS[status] : "never deployed";
  return (
    <span
      className={`inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border px-2.5 py-0.5 text-[11px] font-medium ${s.pill}`}
    >
      <span className="relative flex h-1.5 w-1.5">
        {s.active && (
          <span
            className={`absolute inline-flex h-full w-full animate-ping rounded-full opacity-60 ${s.dot}`}
          />
        )}
        <span
          className={`relative inline-flex h-1.5 w-1.5 rounded-full ${s.dot}`}
        />
      </span>
      {label}
    </span>
  );
}
