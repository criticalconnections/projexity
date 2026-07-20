import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { motion } from "motion/react";
import { Boxes, Plus, RefreshCw, Server, Unplug } from "lucide-react";
import { api, parseBootstrapSteps, type Target } from "../api";
import { EmptyState } from "../components/EmptyState";

export function TargetsPage() {
  const queryClient = useQueryClient();
  const { data: targets, isLoading } = useQuery({
    queryKey: ["targets"],
    queryFn: api.listTargets,
    refetchInterval: (q) =>
      q.state.data?.some((t) => t.status === "bootstrapping") ? 2000 : false,
  });

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteTarget(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["targets"] }),
  });

  const repair = useMutation({
    mutationFn: (id: string) => api.bootstrapTarget(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["targets"] }),
  });

  return (
    <div>
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Targets</h1>
          <p className="mt-1 text-sm text-zinc-500">
            Servers and clusters your apps deploy onto.
          </p>
        </div>
        <Link to="/targets/connect" className="btn-primary shrink-0">
          <Plus className="h-4 w-4" strokeWidth={1.75} />
          Connect target
        </Link>
      </div>

      <div className="mt-8">
        {isLoading ? null : !targets || targets.length === 0 ? (
          <EmptyState
            icon={Server}
            title="No targets connected"
            description="Connect a server over SSH and Projexity will install Docker and a reverse proxy with automatic HTTPS — or point it at your Kubernetes cluster."
            action={
              <Link to="/targets/connect" className="btn-primary">
                <Plus className="h-4 w-4" strokeWidth={1.75} />
                Connect your first target
              </Link>
            }
          />
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {targets.map((t, i) => (
              <motion.div
                key={t.id}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.03, duration: 0.22, ease: "easeOut" }}
              >
                <TargetCard
                  target={t}
                  onDelete={() => {
                    if (
                      window.confirm(
                        `Disconnect ${t.name}? Nothing is removed from the server itself.`,
                      )
                    ) {
                      remove.mutate(t.id);
                    }
                  }}
                  onRepair={() => repair.mutate(t.id)}
                />
              </motion.div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

const STATUS_STYLES: Record<
  Target["status"],
  { pill: string; dot: string; active: boolean }
> = {
  ready: {
    pill: "border-emerald-500/25 bg-emerald-500/10 text-emerald-300",
    dot: "bg-emerald-400",
    active: false,
  },
  bootstrapping: {
    pill: "border-amber-500/25 bg-amber-500/10 text-amber-300",
    dot: "bg-amber-400",
    active: true,
  },
  pending: {
    pill: "border-white/10 bg-white/[0.04] text-zinc-400",
    dot: "bg-zinc-500",
    active: false,
  },
  error: {
    pill: "border-red-500/25 bg-red-500/10 text-red-300",
    dot: "bg-red-400",
    active: false,
  },
};

const STATUS_LABELS: Record<Target["status"], string> = {
  ready: "ready",
  bootstrapping: "setting up…",
  pending: "not set up",
  error: "setup failed",
};

function TargetStatusPill({ status }: { status: Target["status"] }) {
  const s = STATUS_STYLES[status];
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
      {STATUS_LABELS[status]}
    </span>
  );
}

function TargetCard({
  target,
  onDelete,
  onRepair,
}: {
  target: Target;
  onDelete: () => void;
  onRepair: () => void;
}) {
  const isDocker = target.kind === "docker_server";
  const facts = isDocker ? target.facts : null;
  const info = target.cluster?.info ?? null;
  const failedStep = parseBootstrapSteps(target.status_detail).find(
    (s) => s.status === "failed",
  );
  const Icon = isDocker ? Server : Boxes;
  const meta = isDocker
    ? `${target.ssh_user}@${target.host}${
        target.port !== 22 ? `:${target.port}` : ""
      }`
    : info
      ? [
          `Kubernetes ${info.version}`,
          `${info.node_count} node${info.node_count === 1 ? "" : "s"}`,
          target.cluster?.ingress_class || info.ingress_classes.join(", "),
        ]
          .filter(Boolean)
          .join(" · ")
      : "Kubernetes cluster";
  return (
    <div className="card card-hover p-5">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-white/[0.06] bg-white/[0.04] text-zinc-400">
            <Icon className="h-[18px] w-[18px]" strokeWidth={1.75} />
          </span>
          <div className="min-w-0">
            <h2 className="truncate font-medium tracking-tight text-zinc-100">
              {target.name}
            </h2>
            <p className="mt-0.5 truncate font-mono text-[13px] text-zinc-500">
              {meta}
            </p>
          </div>
        </div>
        <TargetStatusPill status={target.status} />
      </div>

      {facts && (
        <p className="mt-3 truncate font-mono text-xs text-zinc-500">
          {[facts.distro ?? facts.os, facts.arch, facts.docker_version]
            .filter(Boolean)
            .join(" · ")}
        </p>
      )}
      {target.status === "error" && failedStep && (
        <p className="mt-3 rounded-md border border-red-500/20 bg-red-500/5 px-3 py-2 text-xs text-red-300">
          {failedStep.label}: {failedStep.detail || "failed"}
        </p>
      )}

      <div className="mt-4 flex gap-2">
        {isDocker && (target.status === "error" || target.status === "ready") && (
          <button
            onClick={onRepair}
            className="btn-secondary px-3 py-1.5 text-xs"
          >
            <RefreshCw className="h-3.5 w-3.5" strokeWidth={1.75} />
            {target.status === "error" ? "Retry setup" : "Repair"}
          </button>
        )}
        <button onClick={onDelete} className="btn-danger-ghost text-xs">
          <Unplug className="h-3.5 w-3.5" strokeWidth={1.75} />
          Disconnect
        </button>
      </div>
    </div>
  );
}
