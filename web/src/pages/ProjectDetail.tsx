import { useCallback, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import {
  ArrowLeft,
  Check,
  Globe,
  Plus,
  Rocket,
  RotateCcw,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import {
  api,
  ApiError,
  deploymentLogsUrl,
  isDeploymentActive,
  runtimeLogsUrl,
  type Deployment,
  type EnvVar,
} from "../api";
import { LogPanel, useSseLog } from "../components/LogPanel";
import { StatusPill } from "../components/StatusPill";
import { timeAgo } from "../time";

type Tab = "deployments" | "env" | "runtime";

export function ProjectDetailPage({ id }: { id: string }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<Tab>("deployments");
  const [logId, setLogId] = useState<string | null>(null);
  const [deployError, setDeployError] = useState<string | null>(null);

  const projectQuery = useQuery({
    queryKey: ["project", id],
    queryFn: () => api.getProject(id),
    refetchInterval: (q) =>
      isDeploymentActive(q.state.data?.latest_deployment) ? 2000 : false,
  });
  const project = projectQuery.data;
  const latest = project?.latest_deployment ?? null;
  const latestActive = isDeploymentActive(latest);

  const deploymentsQuery = useQuery({
    queryKey: ["deployments", id],
    queryFn: () => api.listProjectDeployments(id),
    refetchInterval: (q) =>
      q.state.data?.some((d) => isDeploymentActive(d)) ? 2000 : false,
  });
  const deployments = deploymentsQuery.data ?? [];

  // Auto-open the live log when a deployment starts.
  const latestId = latest?.id ?? null;
  useEffect(() => {
    if (latestId && latestActive) setLogId(latestId);
  }, [latestId, latestActive]);

  const refetchAll = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["project", id] });
    queryClient.invalidateQueries({ queryKey: ["deployments", id] });
    queryClient.invalidateQueries({ queryKey: ["projects"] });
  }, [queryClient, id]);

  const log = useSseLog(logId ? deploymentLogsUrl(logId) : null, refetchAll);

  const deploy = useMutation({
    mutationFn: () => api.deployProject(id),
    onSuccess: (d) => {
      setDeployError(null);
      setLogId(d.id);
      refetchAll();
    },
    onError: (e) =>
      setDeployError(
        e instanceof ApiError ? e.message : "Something went wrong",
      ),
  });

  const rollback = useMutation({
    mutationFn: (deploymentId: string) => api.rollbackDeployment(deploymentId),
    onSuccess: (d) => {
      setDeployError(null);
      setLogId(d.id);
      refetchAll();
    },
    onError: (e) =>
      setDeployError(
        e instanceof ApiError ? e.message : "Something went wrong",
      ),
  });

  const remove = useMutation({
    mutationFn: () => api.deleteProject(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["projects"] });
      navigate({ to: "/" });
    },
    onError: (e) =>
      setDeployError(
        e instanceof ApiError ? e.message : "Couldn't delete the project",
      ),
  });

  if (projectQuery.isLoading) {
    return (
      <div className="flex items-center gap-3 text-sm text-zinc-500">
        <span className="h-4 w-4 animate-spin rounded-full border-2 border-white/10 border-t-emerald-400" />
        Loading…
      </div>
    );
  }
  if (!project) {
    return (
      <div>
        <p className="text-zinc-300">This project doesn't exist (anymore).</p>
        <Link
          to="/"
          className="mt-3 inline-flex items-center gap-1.5 text-sm text-emerald-400 hover:underline"
        >
          <ArrowLeft className="h-3.5 w-3.5" strokeWidth={1.75} />
          Back to projects
        </Link>
      </div>
    );
  }

  const logDeployment = deployments.find((d) => d.id === logId) ?? null;

  return (
    <div>
      <Link
        to="/"
        className="group inline-flex items-center gap-1.5 text-sm text-zinc-500 transition-colors hover:text-zinc-300"
      >
        <ArrowLeft
          className="h-3.5 w-3.5 transition-transform duration-150 group-hover:-translate-x-0.5"
          strokeWidth={1.75}
        />
        Projects
      </Link>

      {/* header */}
      <div className="mt-3 flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-3">
            <h1 className="truncate text-2xl font-semibold tracking-tight text-zinc-100">
              {project.name}
            </h1>
            <StatusPill status={latest?.status ?? null} />
          </div>
          <p className="mt-1 font-mono text-[13px] text-zinc-500">
            {project.image ?? "no image"} · port {project.container_port}
          </p>
          {project.domains.length > 0 && (
            <div className="mt-2.5 flex flex-wrap gap-2">
              {project.domains.map((d) => (
                <a
                  key={d}
                  href={`https://${d}`}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-1.5 rounded-md border border-white/[0.06] bg-white/[0.03] px-2 py-1 font-mono text-xs text-emerald-400 transition-colors hover:border-emerald-500/40 hover:text-emerald-300"
                >
                  <Globe className="h-3 w-3" strokeWidth={1.75} />
                  {d}
                </a>
              ))}
            </div>
          )}
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            onClick={() => deploy.mutate()}
            disabled={deploy.isPending || latestActive}
            className="btn-primary"
          >
            <Rocket className="h-4 w-4" strokeWidth={1.75} />
            {latestActive
              ? "Deploying…"
              : deploy.isPending
                ? "Starting…"
                : "Deploy"}
          </button>
          <button
            onClick={() => {
              if (
                window.confirm(
                  `Delete ${project.name}? Its container is stopped and removed from the server.`,
                )
              ) {
                remove.mutate();
              }
            }}
            disabled={remove.isPending}
            title="Delete project"
            className="btn-danger-ghost px-3 py-2"
          >
            <Trash2 className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </div>
      </div>

      {deployError && (
        <p className="mt-3 rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-sm text-amber-300">
          {deployError}
        </p>
      )}
      {latest?.status === "failed" && latest.error && (
        <p className="mt-3 rounded-md border border-red-500/20 bg-red-500/5 px-3 py-2 text-sm text-red-300">
          Last deployment failed: {latest.error}
        </p>
      )}

      {/* live deployment log */}
      <AnimatePresence>
        {logId && (
          <motion.section
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.25, ease: "easeOut" }}
            className="overflow-hidden"
          >
            <div className="mt-8">
              <div className="mb-2 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <h2 className="text-sm font-medium text-zinc-300">
                    Deployment{" "}
                    <span className="font-mono text-zinc-500">
                      {(logDeployment ?? latest)?.release_spec.release_id.slice(
                        0,
                        8,
                      ) ?? logId.slice(0, 8)}
                    </span>
                  </h2>
                  <StatusPill
                    status={(logDeployment ?? latest)?.status ?? null}
                  />
                </div>
                <button
                  onClick={() => setLogId(null)}
                  className="btn-ghost px-2 py-1 text-xs"
                >
                  Hide log
                  <X className="h-3.5 w-3.5" strokeWidth={1.75} />
                </button>
              </div>
              <LogPanel
                lines={log.lines}
                connecting={log.connecting}
                error={log.error}
                title={`deploy · ${
                  (logDeployment ?? latest)?.release_spec.release_id.slice(
                    0,
                    8,
                  ) ?? logId.slice(0, 8)
                }`}
                live={!log.ended && !log.error}
              />
              {log.ended && (
                <p className="mt-2 text-xs text-zinc-500">
                  Stream ended — final status:{" "}
                  <span className="text-zinc-300">{log.ended}</span>
                </p>
              )}
            </div>
          </motion.section>
        )}
      </AnimatePresence>

      {/* tabs */}
      <div className="mt-8 flex gap-1 border-b border-white/[0.06]">
        {(
          [
            ["deployments", "Deployments"],
            ["env", "Environment"],
            ["runtime", "Runtime logs"],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            className={`relative -mb-px px-4 py-2.5 text-sm transition-colors duration-150 ${
              tab === key ? "text-zinc-100" : "text-zinc-500 hover:text-zinc-300"
            }`}
          >
            {label}
            {tab === key && (
              <motion.span
                layoutId="project-tab-underline"
                className="absolute inset-x-2 bottom-0 h-0.5 rounded-full bg-emerald-400"
                transition={{ type: "spring", stiffness: 500, damping: 40 }}
              />
            )}
          </button>
        ))}
      </div>

      <div className="mt-6">
        {tab === "deployments" && (
          <DeploymentList
            deployments={deployments}
            loading={deploymentsQuery.isLoading}
            selectedId={logId}
            onSelect={setLogId}
            onRollback={(deploymentId) => rollback.mutate(deploymentId)}
            rollbackBusy={rollback.isPending}
          />
        )}
        {tab === "env" && <EnvEditor projectId={id} />}
        {tab === "runtime" && <RuntimeLogs projectId={id} />}
      </div>
    </div>
  );
}

/* ---------- deployments tab ---------- */

function DeploymentList({
  deployments,
  loading,
  selectedId,
  onSelect,
  onRollback,
  rollbackBusy,
}: {
  deployments: Deployment[];
  loading: boolean;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onRollback: (id: string) => void;
  rollbackBusy: boolean;
}) {
  if (loading) return <p className="text-sm text-zinc-500">Loading…</p>;
  if (deployments.length === 0) {
    return (
      <p className="text-sm text-zinc-500">
        No deployments yet — hit Deploy to launch the first one.
      </p>
    );
  }
  return (
    <div className="card divide-y divide-white/[0.04]">
      {deployments.map((d) => (
        <button
          key={d.id}
          onClick={() => onSelect(d.id)}
          className={`group flex w-full flex-wrap items-center gap-x-4 gap-y-1 px-4 py-3 text-left text-sm transition-colors duration-150 first:rounded-t-xl last:rounded-b-xl hover:bg-white/[0.03] ${
            d.id === selectedId ? "bg-white/[0.03]" : ""
          }`}
        >
          <span className="chip-mono w-20 text-center">
            {d.release_spec.release_id.slice(0, 8)}
          </span>
          <span className="min-w-0 flex-1 truncate font-mono text-[13px] text-zinc-500">
            {d.release_spec.image}
          </span>
          {d.kind === "rollback" && (
            <span className="rounded border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400">
              rollback
            </span>
          )}
          {d.status === "superseded" && (
            <span
              role="button"
              tabIndex={0}
              onClick={(e) => {
                e.stopPropagation();
                if (!rollbackBusy) onRollback(d.id);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.stopPropagation();
                  if (!rollbackBusy) onRollback(d.id);
                }
              }}
              className={`inline-flex items-center gap-1 rounded-md border border-white/10 px-2 py-0.5 text-xs text-zinc-300 opacity-0 transition-all duration-150 focus:opacity-100 group-hover:opacity-100 hover:border-emerald-500/60 hover:text-emerald-300 ${
                rollbackBusy ? "pointer-events-none opacity-40" : ""
              }`}
            >
              <RotateCcw className="h-3 w-3" strokeWidth={1.75} />
              Roll back
            </span>
          )}
          <StatusPill status={d.status} />
          <span className="w-20 text-right text-xs text-zinc-600">
            {timeAgo(d.created_at)}
          </span>
          {d.status === "failed" && d.error && (
            <span className="w-full truncate text-xs text-red-400">
              {d.error}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

/* ---------- environment tab ---------- */

function EnvEditor({ projectId }: { projectId: string }) {
  const queryClient = useQueryClient();
  const envQuery = useQuery({
    queryKey: ["env", projectId],
    queryFn: () => api.getProjectEnv(projectId),
  });
  const [draft, setDraft] = useState<EnvVar[] | null>(null);
  const [saved, setSaved] = useState(false);

  const rows = draft ?? envQuery.data ?? [];
  const edit = (next: EnvVar[]) => {
    setDraft(next);
    setSaved(false);
  };

  const save = useMutation({
    mutationFn: (vars: EnvVar[]) => api.putProjectEnv(projectId, vars),
    onSuccess: (_data, vars) => {
      queryClient.setQueryData(["env", projectId], vars);
      setDraft(null);
      setSaved(true);
    },
  });

  if (envQuery.isLoading) {
    return <p className="text-sm text-zinc-500">Loading…</p>;
  }

  const cleaned = rows
    .filter((r) => r.key.trim() !== "")
    .map((r) => ({ ...r, key: r.key.trim() }));

  return (
    <div className="max-w-3xl">
      {rows.length === 0 ? (
        <p className="text-sm text-zinc-500">No environment variables yet.</p>
      ) : (
        <div className="space-y-2">
          {rows.map((row, i) => (
            <div key={i} className="flex items-center gap-2">
              <input
                value={row.key}
                onChange={(e) =>
                  edit(rows.map((r, j) => (j === i ? { ...r, key: e.target.value } : r)))
                }
                placeholder="KEY"
                spellCheck={false}
                autoComplete="off"
                className="input w-52 font-mono"
              />
              <input
                value={row.value}
                onChange={(e) =>
                  edit(rows.map((r, j) => (j === i ? { ...r, value: e.target.value } : r)))
                }
                placeholder="value"
                spellCheck={false}
                autoComplete="off"
                className="input min-w-0 flex-1 font-mono"
              />
              <button
                onClick={() => edit(rows.filter((_, j) => j !== i))}
                title="Remove variable"
                className="btn-danger-ghost px-2.5 py-2"
              >
                <X className="h-4 w-4" strokeWidth={1.75} />
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="mt-4 flex items-center gap-3">
        <button
          onClick={() =>
            edit([...rows, { key: "", value: "", is_build_time: false }])
          }
          className="inline-flex items-center gap-1.5 rounded-md border border-dashed border-white/10 px-3 py-1.5 text-sm text-zinc-400 transition-colors duration-150 hover:border-white/25 hover:text-zinc-200"
        >
          <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
          Add variable
        </button>
        <button
          onClick={() => save.mutate(cleaned)}
          disabled={save.isPending || draft === null}
          className="btn-primary px-4 py-1.5"
        >
          {save.isPending ? "Saving…" : "Save"}
        </button>
        {saved && (
          <motion.span
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            className="inline-flex items-center gap-1 text-sm text-emerald-400"
          >
            <Check className="h-4 w-4" strokeWidth={2} />
            Saved
          </motion.span>
        )}
        {save.error && (
          <span className="text-sm text-red-400">
            {save.error instanceof ApiError
              ? save.error.message
              : "Couldn't save"}
          </span>
        )}
      </div>
      <p className="mt-3 text-xs text-zinc-600">
        Applied on next deploy.
      </p>
    </div>
  );
}

/* ---------- runtime logs tab ---------- */

function RuntimeLogs({ projectId }: { projectId: string }) {
  const [streaming, setStreaming] = useState(false);
  const log = useSseLog(streaming ? runtimeLogsUrl(projectId) : null);

  return (
    <div>
      <div className="mb-3 flex items-center gap-3">
        <button
          onClick={() => setStreaming((s) => !s)}
          className={streaming ? "btn-secondary" : "btn-primary"}
        >
          <Terminal className="h-4 w-4" strokeWidth={1.75} />
          {streaming ? "Stop" : "Stream logs"}
        </button>
        {streaming && !log.error && !log.ended && (
          <span className="flex items-center gap-2 text-xs text-zinc-500">
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-60" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-400" />
            </span>
            live
          </span>
        )}
      </div>
      {streaming && (
        <LogPanel
          lines={log.lines}
          connecting={log.connecting}
          error={
            log.error
              ? "Couldn't stream runtime logs — there may be no running container for this project."
              : null
          }
          emptyText="Waiting for container output…"
          title="runtime · stdout/stderr"
          live={!log.error && !log.ended}
        />
      )}
      {!streaming && (
        <p className="text-sm text-zinc-500">
          Stream stdout/stderr from the running container in real time.
        </p>
      )}
    </div>
  );
}
