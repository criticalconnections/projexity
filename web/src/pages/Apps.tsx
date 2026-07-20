import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import {
  AlertTriangle,
  ArrowUpRight,
  Check,
  Globe,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import {
  api,
  ApiError,
  parseBootstrapSteps,
  type AppInstall,
  type CatalogEntry,
} from "../api";
import { AppIcon } from "../components/AppIcon";
import { StepIcon } from "../components/StepIcon";
import { timeAgo } from "../time";

/** One-click apps: installed apps with live install progress on top, the
 * template catalog below, and a modal install dialog. */
export function AppsPage() {
  const [installing, setInstalling] = useState<CatalogEntry | null>(null);
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState<string | null>(null);

  const { data: templates } = useQuery({
    queryKey: ["templates"],
    queryFn: api.listTemplates,
    staleTime: 5 * 60 * 1000,
  });

  const categories = Array.from(
    new Set((templates ?? []).map((t) => t.category)),
  ).sort();
  const q = search.trim().toLowerCase();
  const visible = (templates ?? []).filter(
    (t) =>
      (!category || t.category === category) &&
      (!q ||
        t.name.toLowerCase().includes(q) ||
        t.description.toLowerCase().includes(q) ||
        t.category.toLowerCase().includes(q)),
  );

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
          <h1 className="text-2xl font-semibold tracking-tight">Apps</h1>
          <p className="mt-1 text-sm text-zinc-500">
            One-click open-source apps, installed straight onto your servers.
          </p>
        </div>
      </div>

      {hasApps && (
        <section className="mt-8">
          <h2 className="text-[11px] font-medium uppercase tracking-wider text-zinc-500">
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
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="text-[11px] font-medium uppercase tracking-wider text-zinc-500">
            Catalog
          </h2>
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search apps…"
            spellCheck={false}
            className="input w-56 py-1.5"
          />
        </div>
        {!hasApps && !appsLoading && (
          <p className="mt-1 text-sm text-zinc-500">
            Apps you install appear here with their live URLs.
          </p>
        )}
        {categories.length > 1 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            <button
              onClick={() => setCategory(null)}
              className={`rounded-full border px-2.5 py-1 text-[11px] font-medium uppercase tracking-wider transition ${
                category === null
                  ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-300"
                  : "border-white/[0.08] text-zinc-500 hover:border-white/20 hover:text-zinc-300"
              }`}
            >
              All
            </button>
            {categories.map((c) => (
              <button
                key={c}
                onClick={() => setCategory(category === c ? null : c)}
                className={`rounded-full border px-2.5 py-1 text-[11px] font-medium uppercase tracking-wider transition ${
                  category === c
                    ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-300"
                    : "border-white/[0.08] text-zinc-500 hover:border-white/20 hover:text-zinc-300"
                }`}
              >
                {c}
              </button>
            ))}
          </div>
        )}
        <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {visible.map((t, i) => (
            <CatalogCard
              key={t.id}
              entry={t}
              index={i}
              installedCount={
                (apps ?? []).filter((a) => a.template_id === t.id).length
              }
              onInstall={() => setInstalling(t)}
            />
          ))}
        </div>
        {visible.length === 0 && (templates ?? []).length > 0 && (
          <p className="mt-6 text-sm text-zinc-500">
            Nothing matches “{search}” — try a different search.
          </p>
        )}
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

const APP_STATUS_STYLES: Record<
  AppInstall["status"],
  { pill: string; dot: string; active: boolean }
> = {
  running: {
    pill: "border-emerald-500/25 bg-emerald-500/10 text-emerald-300",
    dot: "bg-emerald-400",
    active: false,
  },
  installing: {
    pill: "border-amber-500/25 bg-amber-500/10 text-amber-300",
    dot: "bg-amber-400",
    active: true,
  },
  error: {
    pill: "border-red-500/25 bg-red-500/10 text-red-300",
    dot: "bg-red-400",
    active: false,
  },
  removing: {
    pill: "border-white/10 bg-white/[0.04] text-zinc-400",
    dot: "bg-zinc-500",
    active: true,
  },
};

const APP_STATUS_LABELS: Record<AppInstall["status"], string> = {
  running: "running",
  installing: "installing…",
  error: "install failed",
  removing: "removing…",
};

function AppStatusPill({ status }: { status: AppInstall["status"] }) {
  const s = APP_STATUS_STYLES[status];
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
      {APP_STATUS_LABELS[status]}
    </span>
  );
}

function InstalledCard({
  app,
  template,
}: {
  app: AppInstall;
  template: CatalogEntry | undefined;
}) {
  const [confirming, setConfirming] = useState(false);

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
      className="card p-5"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <AppIcon icon={template?.icon} name={app.name} size="md" />
          <div className="min-w-0">
            <h3 className="truncate font-medium tracking-tight text-zinc-100">
              {app.name}
            </h3>
            <p className="mt-0.5 text-xs text-zinc-500">
              {template?.name ?? app.template_id} · installed{" "}
              {timeAgo(app.created_at)}
            </p>
          </div>
        </div>
        <AppStatusPill status={app.status} />
      </div>

      {domains.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {domains.map(([service, hostname]) => (
            <a
              key={service}
              href={`https://${hostname}`}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1.5 rounded-md border border-white/[0.06] bg-white/[0.03] px-2.5 py-1 font-mono text-xs text-emerald-400 transition-colors hover:border-emerald-500/40 hover:text-emerald-300"
            >
              <Globe className="h-3 w-3" strokeWidth={1.75} />
              {hostname}
            </a>
          ))}
        </div>
      )}

      {app.status === "installing" && steps.length > 0 && (
        <div className="mt-4 space-y-1.5 rounded-lg border border-white/[0.06] bg-black/30 p-3">
          {steps.map((s) => (
            <div key={s.id} className="flex items-center gap-2 text-xs">
              <StepIcon status={s.status} size="sm" />
              <span
                className={
                  s.status === "pending" ? "text-zinc-600" : "text-zinc-300"
                }
              >
                {s.label}
              </span>
              {s.status === "running" && s.detail && (
                <span className="min-w-0 truncate font-mono text-zinc-500">
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
            onClick={() => setConfirming(true)}
            className="btn-danger-ghost text-xs"
          >
            <Trash2 className="h-3.5 w-3.5" strokeWidth={1.75} />
            Uninstall
          </button>
        </div>
      )}

      <AnimatePresence>
        {confirming && (
          <UninstallDialog app={app} onClose={() => setConfirming(false)} />
        )}
      </AnimatePresence>
    </motion.div>
  );
}

/* ---------- uninstall dialog ---------- */

function UninstallDialog({
  app,
  onClose,
}: {
  app: AppInstall;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [purge, setPurge] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const uninstall = useMutation({
    mutationFn: () => api.uninstallApp(app.id, purge),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
      onClose();
    },
  });

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.15 }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
      onClick={onClose}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.98, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.98 }}
        transition={{ duration: 0.18, ease: "easeOut" }}
        onClick={(e) => e.stopPropagation()}
        className="card w-full max-w-md p-6"
      >
        <div className="flex items-start gap-3">
          <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-red-500/20 bg-red-500/10 text-red-400">
            <Trash2 className="h-4 w-4" strokeWidth={1.75} />
          </span>
          <div>
            <h3 className="font-medium tracking-tight text-zinc-100">
              Uninstall {app.name}?
            </h3>
            <p className="mt-1 text-sm text-zinc-500">
              Its containers and HTTPS routes are removed from the server.
            </p>
          </div>
        </div>

        <label
          className={`mt-5 flex cursor-pointer items-start gap-3 rounded-lg border p-3 transition-colors ${
            purge
              ? "border-red-500/30 bg-red-500/[0.06]"
              : "border-white/[0.08] bg-white/[0.02] hover:border-white/20"
          }`}
        >
          <input
            type="checkbox"
            checked={purge}
            onChange={(e) => setPurge(e.target.checked)}
            className="mt-0.5 h-4 w-4 accent-red-500"
          />
          <span className="text-sm">
            <span className="font-medium text-zinc-200">
              Also delete all data
            </span>
            <span className="mt-0.5 block text-xs text-zinc-500">
              Wipes this app's data volumes on the server. This can't be undone.
            </span>
          </span>
        </label>

        {purge && (
          <p className="mt-3 flex items-center gap-1.5 text-xs text-amber-400">
            <AlertTriangle className="h-3.5 w-3.5" strokeWidth={1.75} />
            Everything this app stored will be permanently erased.
          </p>
        )}

        <div className="mt-6 flex justify-end gap-2">
          <button onClick={onClose} className="btn-ghost text-sm">
            Cancel
          </button>
          <button
            onClick={() => uninstall.mutate()}
            disabled={uninstall.isPending}
            className={
              purge
                ? "btn-primary bg-none px-4 py-2 text-sm"
                : "btn-secondary text-sm"
            }
            style={
              purge
                ? { backgroundImage: "linear-gradient(135deg,#ef4444,#b91c1c)" }
                : undefined
            }
          >
            {uninstall.isPending
              ? "Removing…"
              : purge
                ? "Uninstall & delete data"
                : "Uninstall"}
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

/* ---------- catalog ---------- */

function CatalogCard({
  entry,
  index,
  installedCount,
  onInstall,
}: {
  entry: CatalogEntry;
  index: number;
  installedCount: number;
  onInstall: () => void;
}) {
  const isInstalled = installedCount > 0;
  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.03, duration: 0.25, ease: "easeOut" }}
      className={`card card-hover group flex flex-col p-5 ${
        isInstalled ? "border-emerald-500/25 bg-emerald-500/[0.03]" : ""
      }`}
    >
      <div className="flex items-start justify-between gap-3">
        <AppIcon icon={entry.icon} name={entry.name} size="lg" />
        {isInstalled ? (
          <span className="inline-flex items-center gap-1 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1 text-[10px] font-medium uppercase tracking-wider text-emerald-300">
            <Check className="h-3 w-3" strokeWidth={2.25} />
            Installed{installedCount > 1 ? ` ×${installedCount}` : ""}
          </span>
        ) : (
          <span className="rounded-full border border-white/10 bg-white/[0.03] px-2.5 py-1 text-[10px] font-medium uppercase tracking-wider text-zinc-400">
            {entry.category}
          </span>
        )}
      </div>
      <h3 className="mt-3 font-medium tracking-tight text-zinc-100">
        {entry.name}
      </h3>
      <p className="mt-1 line-clamp-2 text-sm text-zinc-500">
        {entry.description}
      </p>
      <div className="mt-4 flex flex-1 items-end justify-between">
        {isInstalled ? (
          <button
            onClick={onInstall}
            className="btn-secondary px-3.5 py-1.5 text-xs"
          >
            <Plus className="h-3.5 w-3.5" strokeWidth={1.75} />
            Install another
          </button>
        ) : (
          <button
            onClick={onInstall}
            className="btn-primary px-3.5 py-1.5 opacity-80 transition-opacity group-hover:opacity-100"
          >
            Install
          </button>
        )}
        <a
          href={entry.website}
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-0.5 text-xs text-zinc-500 transition-colors hover:text-zinc-300"
        >
          website
          <ArrowUpRight className="h-3 w-3" strokeWidth={1.75} />
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
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.98, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.98, y: 8 }}
        transition={{ duration: 0.18, ease: "easeOut" }}
        className="card max-h-[85vh] w-full max-w-md overflow-y-auto rounded-2xl border-white/[0.08] bg-[#111113] p-6 shadow-2xl shadow-black/60"
      >
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <AppIcon icon={template.icon} name={template.name} size="md" />
            <div>
              <h2 className="text-lg font-semibold tracking-tight text-zinc-100">
                Install {template.name}
              </h2>
              <p className="text-[10px] font-medium uppercase tracking-wider text-zinc-500">
                {template.category}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            title="Close"
            className="btn-ghost px-1.5 py-1.5"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
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
              className="input mt-1"
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
                className="input mt-1"
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
                className="input mt-1 font-mono"
              />
            </Field>
          ))}

          {error && <p className="text-sm text-red-400">{error}</p>}

          <div className="flex items-center justify-end gap-3 pt-2">
            <button type="button" onClick={onClose} className="btn-ghost px-4 py-2">
              Cancel
            </button>
            <button
              type="submit"
              disabled={install.isPending || readyTargets.length === 0}
              className="btn-primary"
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
