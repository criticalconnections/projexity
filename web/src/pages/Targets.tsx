import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
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
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Targets</h1>
          <p className="mt-1 text-sm text-zinc-500">
            Servers and clusters your apps deploy onto.
          </p>
        </div>
        <Link
          to="/targets/new"
          className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
        >
          Connect server
        </Link>
      </div>

      <div className="mt-8">
        {isLoading ? null : !targets || targets.length === 0 ? (
          <EmptyStateWithLink />
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {targets.map((t) => (
              <TargetCard
                key={t.id}
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
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function EmptyStateWithLink() {
  return (
    <div className="relative">
      <EmptyState
        title="No targets connected"
        description="Connect a server over SSH and Projexity will install Docker and a reverse proxy with automatic HTTPS — or point it at your Kubernetes cluster."
      />
      <div className="absolute inset-x-0 bottom-16 flex justify-center">
        <Link
          to="/targets/new"
          className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
        >
          Connect your first server
        </Link>
      </div>
    </div>
  );
}

const STATUS_STYLES: Record<Target["status"], string> = {
  ready: "bg-emerald-500/10 text-emerald-400",
  bootstrapping: "bg-amber-500/10 text-amber-400 animate-pulse",
  pending: "bg-zinc-500/10 text-zinc-400",
  error: "bg-red-500/10 text-red-400",
};

const STATUS_LABELS: Record<Target["status"], string> = {
  ready: "ready",
  bootstrapping: "setting up…",
  pending: "not set up",
  error: "setup failed",
};

function TargetCard({
  target,
  onDelete,
  onRepair,
}: {
  target: Target;
  onDelete: () => void;
  onRepair: () => void;
}) {
  const facts = target.facts;
  const failedStep = parseBootstrapSteps(target.status_detail).find(
    (s) => s.status === "failed",
  );
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/40 p-5">
      <div className="flex items-start justify-between">
        <div>
          <h2 className="font-medium text-zinc-100">{target.name}</h2>
          <p className="mt-0.5 text-sm text-zinc-500">
            {target.ssh_user}@{target.host}
            {target.port !== 22 ? `:${target.port}` : ""}
          </p>
        </div>
        <span
          className={`rounded-full px-2.5 py-1 text-xs font-medium ${STATUS_STYLES[target.status]}`}
        >
          {STATUS_LABELS[target.status]}
        </span>
      </div>

      {facts && (
        <p className="mt-3 text-xs text-zinc-500">
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
        {(target.status === "error" || target.status === "ready") && (
          <button
            onClick={onRepair}
            className="rounded-md border border-zinc-700 px-3 py-1.5 text-xs text-zinc-300 transition hover:border-zinc-500"
          >
            {target.status === "error" ? "Retry setup" : "Repair"}
          </button>
        )}
        <button
          onClick={onDelete}
          className="rounded-md border border-zinc-800 px-3 py-1.5 text-xs text-zinc-500 transition hover:border-red-500/40 hover:text-red-400"
        >
          Disconnect
        </button>
      </div>
    </div>
  );
}
