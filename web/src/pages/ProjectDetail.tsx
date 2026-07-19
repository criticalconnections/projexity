import { useCallback, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
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
    return <p className="text-sm text-zinc-500">Loading…</p>;
  }
  if (!project) {
    return (
      <div>
        <p className="text-zinc-300">This project doesn't exist (anymore).</p>
        <Link
          to="/"
          className="mt-3 inline-block text-sm text-emerald-400 hover:underline"
        >
          ← Back to projects
        </Link>
      </div>
    );
  }

  const logDeployment = deployments.find((d) => d.id === logId) ?? null;

  return (
    <div>
      <Link to="/" className="text-sm text-zinc-500 hover:text-zinc-300">
        ← Projects
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
          <p className="mt-1 font-mono text-sm text-zinc-500">
            {project.image ?? "no image"} · port {project.container_port}
          </p>
          {project.domains.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
              {project.domains.map((d) => (
                <a
                  key={d}
                  href={`https://${d}`}
                  target="_blank"
                  rel="noreferrer"
                  className="text-sm text-emerald-400 hover:underline"
                >
                  {d} ↗
                </a>
              ))}
            </div>
          )}
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            onClick={() => deploy.mutate()}
            disabled={deploy.isPending || latestActive}
            className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
          >
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
            className="rounded-md border border-zinc-800 px-4 py-2 text-sm text-zinc-500 transition hover:border-red-500/40 hover:text-red-400 disabled:opacity-40"
          >
            Delete project
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
                  className="rounded-md px-2 py-1 text-xs text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200"
                >
                  Hide log ✕
                </button>
              </div>
              <LogPanel
                lines={log.lines}
                connecting={log.connecting}
                error={log.error}
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
      <div className="mt-8 flex gap-1 border-b border-zinc-800">
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
            className={`-mb-px border-b-2 px-4 py-2.5 text-sm transition ${
              tab === key
                ? "border-emerald-500 text-zinc-100"
                : "border-transparent text-zinc-500 hover:text-zinc-300"
            }`}
          >
            {label}
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
}: {
  deployments: Deployment[];
  loading: boolean;
  selectedId: string | null;
  onSelect: (id: string) => void;
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
    <div className="divide-y divide-zinc-800/70 rounded-xl border border-zinc-800 bg-zinc-900/40">
      {deployments.map((d) => (
        <button
          key={d.id}
          onClick={() => onSelect(d.id)}
          className={`flex w-full flex-wrap items-center gap-x-4 gap-y-1 px-4 py-3 text-left text-sm transition hover:bg-zinc-900/70 ${
            d.id === selectedId ? "bg-zinc-900/70" : ""
          }`}
        >
          <span className="w-20 font-mono text-zinc-300">
            {d.release_spec.release_id.slice(0, 8)}
          </span>
          <span className="min-w-0 flex-1 truncate font-mono text-zinc-500">
            {d.release_spec.image}
          </span>
          {d.kind === "rollback" && (
            <span className="rounded bg-zinc-500/10 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400">
              rollback
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
                className="w-52 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 font-mono text-sm text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500"
              />
              <input
                value={row.value}
                onChange={(e) =>
                  edit(rows.map((r, j) => (j === i ? { ...r, value: e.target.value } : r)))
                }
                placeholder="value"
                spellCheck={false}
                autoComplete="off"
                className="min-w-0 flex-1 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 font-mono text-sm text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500"
              />
              <button
                onClick={() => edit(rows.filter((_, j) => j !== i))}
                title="Remove variable"
                className="rounded-md border border-zinc-800 px-2.5 py-2 text-sm text-zinc-500 transition hover:border-red-500/40 hover:text-red-400"
              >
                ✕
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
          className="rounded-md border border-zinc-700 px-3 py-1.5 text-sm text-zinc-300 transition hover:border-zinc-500"
        >
          + Add variable
        </button>
        <button
          onClick={() => save.mutate(cleaned)}
          disabled={save.isPending || draft === null}
          className="rounded-md bg-emerald-600 px-4 py-1.5 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
        >
          {save.isPending ? "Saving…" : "Save"}
        </button>
        {saved && <span className="text-sm text-emerald-400">Saved ✓</span>}
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
          className={
            streaming
              ? "rounded-md border border-zinc-700 px-4 py-2 text-sm text-zinc-300 transition hover:border-zinc-500"
              : "rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
          }
        >
          {streaming ? "Stop" : "Stream logs"}
        </button>
        {streaming && !log.error && !log.ended && (
          <span className="flex items-center gap-2 text-xs text-zinc-500">
            <span className="h-2 w-2 animate-pulse rounded-full bg-emerald-400" />
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
