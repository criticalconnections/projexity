import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import {
  api,
  ApiError,
  parseBootstrapSteps,
  type AppInstall,
  type CatalogEntry,
} from "../api";
import { timeAgo } from "../time";

/** One-click apps: installed apps with live install progress on top, the
 * template catalog below, and a modal install dialog. */
export function AppsPage() {
  const [installing, setInstalling] = useState<CatalogEntry | null>(null);

  const { data: templates } = useQuery({
    queryKey: ["templates"],
    queryFn: api.listTemplates,
    staleTime: 5 * 60 * 1000,
  });

  const { data: apps, isLoading: appsLoading } = useQuery({
    queryKey: ["apps"],
    queryFn: api.listApps,
    refetchInterval: (q) =>
      q.state.data?.some(
        (a) => a.status === "installing" || a.status === "removing",
      )
        ? 2000
        : false,
  });

  const hasApps = !!apps && apps.length > 0;

  return (
    <div>
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Apps</h1>
          <p className="mt-1 text-sm text-zinc-500">
            One-click open-source apps, installed straight onto your servers.
          </p>
        </div>
      </div>

      {hasApps && (
        <section className="mt-8">
          <h2 className="text-sm font-medium uppercase tracking-wider text-zinc-500">
            Installed
          </h2>
          <div className="mt-3 grid gap-4 md:grid-cols-2">
            <AnimatePresence initial={false}>
              {apps.map((app) => (
                <InstalledCard
                  key={app.id}
                  app={app}
                  template={templates?.find((t) => t.id === app.template_id)}
                />
              ))}
            </AnimatePresence>
          </div>
        </section>
      )}

      <section className="mt-10">
        <h2 className="text-sm font-medium uppercase tracking-wider text-zinc-500">
          Catalog
        </h2>
        {!hasApps && !appsLoading && (
          <p className="mt-1 text-sm text-zinc-500">
            Apps you install appear here with their live URLs.
          </p>
        )}
        <div className="mt-3 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {(templates ?? []).map((t, i) => (
            <CatalogCard
              key={t.id}
              entry={t}
              index={i}
              onInstall={() => setInstalling(t)}
            />
          ))}
        </div>
      </section>

      <AnimatePresence>
        {installing && (
          <InstallDialog
            template={installing}
            onClose={() => setInstalling(null)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

/* ---------- installed apps ---------- */

const APP_STATUS_STYLES: Record<AppInstall["status"], string> = {
  running: "bg-emerald-500/10 text-emerald-400",
  installing: "bg-amber-500/10 text-amber-400 animate-pulse",
  error: "bg-red-500/10 text-red-400",
  removing: "bg-zinc-500/10 text-zinc-400 animate-pulse",
};

const APP_STATUS_LABELS: Record<AppInstall["status"], string> = {
  running: "running",
  installing: "installing…",
  error: "install failed",
  removing: "removing…",
};

function InstalledCard({
  app,
  template,
}: {
  app: AppInstall;
  template: CatalogEntry | undefined;
}) {
  const queryClient = useQueryClient();
  const uninstall = useMutation({
    mutationFn: () => api.uninstallApp(app.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["apps"] }),
  });

  const steps = parseBootstrapSteps(app.status_detail);
  const failedStep = steps.find((s) => s.status === "failed");
  const domains = Object.entries(app.domains);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.97 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className="rounded-xl border border-zinc-800 bg-zinc-900/40 p-5"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <span className="text-2xl" aria-hidden>
            {template?.icon ?? "📦"}
          </span>
          <div className="min-w-0">
            <h3 className="truncate font-medium text-zinc-100">{app.name}</h3>
            <p className="mt-0.5 text-xs text-zinc-500">
              {template?.name ?? app.template_id} · installed{" "}
              {timeAgo(app.created_at)}
            </p>
          </div>
        </div>
        <span
          className={`whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-medium ${APP_STATUS_STYLES[app.status]}`}
        >
          {APP_STATUS_LABELS[app.status]}
        </span>
      </div>

      {domains.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {domains.map(([service, hostname]) => (
            <a
              key={service}
              href={`https://${hostname}`}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-900/60 px-2.5 py-1 text-xs text-emerald-400 transition hover:border-emerald-500/40"
            >
              {hostname}
              <span aria-hidden>↗</span>
            </a>
          ))}
        </div>
      )}

      {app.status === "installing" && steps.length > 0 && (
        <div className="mt-4 space-y-1.5 rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3">
          {steps.map((s) => (
            <div key={s.id} className="flex items-center gap-2 text-xs">
              <StepIcon status={s.status} />
              <span
                className={
                  s.status === "pending" ? "text-zinc-600" : "text-zinc-300"
                }
              >
                {s.label}
              </span>
              {s.status === "running" && s.detail && (
                <span className="min-w-0 truncate text-zinc-500">
                  {s.detail}
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {app.status === "error" && failedStep && (
        <p className="mt-3 rounded-md border border-red-500/20 bg-red-500/5 px-3 py-2 text-xs text-red-300">
          {failedStep.label}: {failedStep.detail || "failed"}
        </p>
      )}

      {app.status !== "removing" && (
        <div className="mt-4">
          <button
            onClick={() => {
              if (
                window.confirm(
                  `Uninstall ${app.name}? Its data volumes are kept on the server.`,
                )
              ) {
                uninstall.mutate();
              }
            }}
            disabled={uninstall.isPending}
            className="rounded-md border border-zinc-800 px-3 py-1.5 text-xs text-zinc-500 transition hover:border-red-500/40 hover:text-red-400 disabled:opacity-40"
          >
            {uninstall.isPending ? "Removing…" : "Uninstall"}
          </button>
        </div>
      )}
    </motion.div>
  );
}

function StepIcon({ status }: { status: string }) {
  if (status === "done" || status === "skipped")
    return (
      <span className="flex h-4 w-4 flex-none items-center justify-center rounded-full bg-emerald-500/15 text-[10px] text-emerald-400">
        ✓
      </span>
    );
  if (status === "failed")
    return (
      <span className="flex h-4 w-4 flex-none items-center justify-center rounded-full bg-red-500/15 text-[10px] text-red-400">
        ✕
      </span>
    );
  if (status === "running")
    return (
      <span className="flex h-4 w-4 flex-none items-center justify-center">
        <span className="h-3 w-3 animate-spin rounded-full border-2 border-zinc-700 border-t-emerald-400" />
      </span>
    );
  return (
    <span className="flex h-4 w-4 flex-none items-center justify-center text-zinc-700">
      ○
    </span>
  );
}

/* ---------- catalog ---------- */

function CatalogCard({
  entry,
  index,
  onInstall,
}: {
  entry: CatalogEntry;
  index: number;
  onInstall: () => void;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.04, duration: 0.25, ease: "easeOut" }}
      className="flex flex-col rounded-xl border border-zinc-800 bg-zinc-900/40 p-5 transition-colors hover:border-zinc-700"
    >
      <div className="flex items-start justify-between gap-3">
        <span className="text-3xl" aria-hidden>
          {entry.icon}
        </span>
        <span className="rounded-full bg-zinc-800/80 px-2.5 py-1 text-[11px] font-medium text-zinc-400">
          {entry.category}
        </span>
      </div>
      <h3 className="mt-3 font-medium text-zinc-100">{entry.name}</h3>
      <p className="mt-1 line-clamp-2 text-sm text-zinc-500">
        {entry.description}
      </p>
      <div className="mt-4 flex flex-1 items-end justify-between">
        <button
          onClick={onInstall}
          className="rounded-md bg-emerald-600 px-3.5 py-1.5 text-sm font-medium text-white transition hover:bg-emerald-500"
        >
          Install
        </button>
        <a
          href={entry.website}
          target="_blank"
          rel="noreferrer"
          className="text-xs text-zinc-500 transition hover:text-zinc-300"
        >
          website ↗
        </a>
      </div>
    </motion.div>
  );
}

/* ---------- install dialog ---------- */

function InstallDialog({
  template,
  onClose,
}: {
  template: CatalogEntry;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [name, setName] = useState(template.name);
  const [targetId, setTargetId] = useState("");
  const [env, setEnv] = useState<Record<string, string>>(() =>
    Object.fromEntries(template.env.map((f) => [f.key, f.default ?? ""])),
  );
  const [error, setError] = useState<string | null>(null);

  const { data: targets } = useQuery({
    queryKey: ["targets"],
    queryFn: api.listTargets,
  });
  const readyTargets = (targets ?? []).filter((t) => t.status === "ready");

  // Preselect the only (or first) ready server.
  useEffect(() => {
    if (!targetId && readyTargets.length > 0) setTargetId(readyTargets[0].id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [readyTargets.length]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const install = useMutation({
    mutationFn: () =>
      api.installApp({
        template_id: template.id,
        target_id: targetId,
        name: name.trim() || undefined,
        env: Object.fromEntries(
          Object.entries(env).filter(([, v]) => v.trim() !== ""),
        ),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
      onClose();
    },
    onError: (e) =>
      setError(e instanceof ApiError ? e.message : "Something went wrong"),
  });

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.15 }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-zinc-950/70 p-4 backdrop-blur-sm"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.95, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.95, y: 8 }}
        transition={{ duration: 0.18, ease: "easeOut" }}
        className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-2xl border border-zinc-800 bg-zinc-900 p-6 shadow-2xl"
      >
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <span className="text-3xl" aria-hidden>
              {template.icon}
            </span>
            <div>
              <h2 className="text-lg font-semibold text-zinc-100">
                Install {template.name}
              </h2>
              <p className="text-xs text-zinc-500">{template.category}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="rounded-md px-2 py-1 text-sm text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
          >
            ✕
          </button>
        </div>

        <form
          className="mt-6 space-y-4"
          onSubmit={(e) => {
            e.preventDefault();
            setError(null);
            if (targetId) install.mutate();
          }}
        >
          <Field label="Name">
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={template.name}
              className="mt-1 w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none transition-colors focus:border-emerald-500"
            />
          </Field>

          <Field label="Server">
            {readyTargets.length === 0 ? (
              <p className="mt-1 rounded-md border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-sm text-amber-300">
                No server is ready yet.{" "}
                <Link
                  to="/targets/new"
                  className="font-medium text-emerald-400 underline underline-offset-2 hover:text-emerald-300"
                >
                  Connect one first
                </Link>
                .
              </p>
            ) : (
              <select
                value={targetId}
                onChange={(e) => setTargetId(e.target.value)}
                required
                className="mt-1 w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none transition-colors focus:border-emerald-500"
              >
                {readyTargets.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name} ({t.host})
                  </option>
                ))}
              </select>
            )}
          </Field>

          {template.env.map((f) => (
            <Field
              key={f.key}
              label={
                <>
                  {f.label ?? f.key}
                  {f.required && <span className="text-emerald-400"> *</span>}
                </>
              }
            >
              <input
                value={env[f.key] ?? ""}
                onChange={(e) =>
                  setEnv((prev) => ({ ...prev, [f.key]: e.target.value }))
                }
                placeholder={f.default ?? ""}
                required={f.required}
                spellCheck={false}
                autoComplete="off"
                className="mt-1 w-full rounded-md border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-200 outline-none transition-colors focus:border-emerald-500"
              />
            </Field>
          ))}

          {error && <p className="text-sm text-red-400">{error}</p>}

          <div className="flex items-center justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md px-4 py-2 text-sm text-zinc-400 transition hover:text-zinc-200"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={install.isPending || readyTargets.length === 0}
              className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
            >
              {install.isPending ? "Installing…" : "Install"}
            </button>
          </div>
        </form>
      </motion.div>
    </motion.div>
  );
}

function Field({
  label,
  children,
}: {
  label: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <label className="block text-sm text-zinc-400">
      {label}
      {children}
    </label>
  );
}
