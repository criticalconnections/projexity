import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy, ExternalLink, FolderGit2 } from "lucide-react";
import { api, type GithubAppStatus } from "../api";

export function SettingsPage() {
  const [connected, setConnected] = useState(false);

  // Handle the ?github=connected redirect from the GitHub App install flow.
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get("github") === "connected") {
      setConnected(true);
      params.delete("github");
      const rest = params.toString();
      window.history.replaceState(
        null,
        "",
        window.location.pathname + (rest ? `?${rest}` : ""),
      );
      const t = setTimeout(() => setConnected(false), 6000);
      return () => clearTimeout(t);
    }
  }, []);

  const status = useQuery({
    queryKey: ["github", "status"],
    queryFn: api.githubStatus,
  });

  return (
    <div>
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
      <p className="mt-1 text-sm text-zinc-500">
        Instance-level configuration and integrations.
      </p>

      <AnimatePresence>
        {connected && (
          <motion.div
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="mt-4 flex items-center gap-2 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-4 py-2.5 text-sm text-emerald-300"
          >
            <Check className="h-4 w-4" strokeWidth={2} />
            GitHub connected
          </motion.div>
        )}
      </AnimatePresence>

      <div className="card mt-8 max-w-2xl p-6">
        <div className="flex items-center gap-3">
          <span className="flex h-9 w-9 items-center justify-center rounded-lg border border-white/[0.06] bg-white/[0.04] text-zinc-400">
            <FolderGit2 className="h-4 w-4" strokeWidth={1.75} />
          </span>
          <h2 className="font-medium tracking-tight text-zinc-100">GitHub</h2>
        </div>
        {status.isLoading ? (
          <p className="mt-4 text-sm text-zinc-500">Loading…</p>
        ) : status.isError || !status.data ? (
          <p className="mt-4 text-sm text-red-400">
            Couldn't load GitHub status.
          </p>
        ) : status.data.configured ? (
          <GithubConfigured status={status.data} />
        ) : (
          <GithubNotConfigured status={status.data} />
        )}
      </div>
    </div>
  );
}

function GithubNotConfigured({ status }: { status: GithubAppStatus }) {
  const create = useMutation({
    mutationFn: async () => {
      const { action_url, manifest } = await api.githubManifest();
      // GitHub's app-manifest flow requires a real browser form POST (full-page
      // navigation), so we build and submit a hidden form.
      const form = document.createElement("form");
      form.method = "post";
      form.action = action_url;
      const input = document.createElement("input");
      input.type = "hidden";
      input.name = "manifest";
      input.value = JSON.stringify(manifest);
      form.appendChild(input);
      document.body.appendChild(form);
      form.submit();
    },
  });

  return (
    <div className="mt-4">
      <p className="text-sm text-zinc-500">
        Register a GitHub App owned by your account to deploy private repos and
        get push-to-deploy.
      </p>
      {status.public_url_is_local && (
        <p className="mt-3 rounded-md border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-xs text-amber-300">
          Your instance URL is{" "}
          <span className="font-mono">{status.webhook_url}</span> — GitHub
          can't deliver webhooks to localhost. Set PJX_PUBLIC_URL (e.g. a
          cloudflared tunnel) before creating the app for push-to-deploy to
          work.
        </p>
      )}
      {create.isError && (
        <p className="mt-3 text-sm text-red-400">
          Couldn't start GitHub App creation. Try again.
        </p>
      )}
      <button
        onClick={() => create.mutate()}
        disabled={create.isPending}
        className="btn-primary mt-4"
      >
        {create.isPending ? "Redirecting to GitHub…" : "Create GitHub App"}
      </button>
    </div>
  );
}

function WebhookUrl({ url }: { url: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return (
    <div className="mt-1 flex items-center gap-2">
      <code className="block min-w-0 flex-1 overflow-x-auto whitespace-nowrap rounded-md border border-white/[0.08] bg-[#0b0b0d] px-3 py-2 font-mono text-xs text-zinc-300">
        {url}
      </code>
      <button
        onClick={copy}
        title="Copy webhook URL"
        className="btn-secondary shrink-0 px-2.5 py-2"
      >
        {copied ? (
          <Check className="h-3.5 w-3.5 text-emerald-400" strokeWidth={2} />
        ) : (
          <Copy className="h-3.5 w-3.5" strokeWidth={1.75} />
        )}
      </button>
    </div>
  );
}

function GithubConfigured({ status }: { status: GithubAppStatus }) {
  const appUrl = status.app_url ?? "";
  const noInstalls = status.installations.length === 0;
  return (
    <div className="mt-4 space-y-4">
      <div className="flex items-center justify-between text-sm">
        <span className="text-zinc-500">App</span>
        {appUrl ? (
          <a
            href={appUrl}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 font-mono text-[13px] text-emerald-400 transition-colors hover:text-emerald-300 hover:underline"
          >
            {status.app_slug}
            <ExternalLink className="h-3 w-3" strokeWidth={1.75} />
          </a>
        ) : (
          <span className="font-mono text-[13px] text-zinc-200">
            {status.app_slug}
          </span>
        )}
      </div>

      <div className="flex items-center justify-between text-sm">
        <span className="text-zinc-500">Installations</span>
        <span className="text-zinc-200">
          {noInstalls
            ? "none yet"
            : `${status.installations.length} — ${status.installations
                .map((i) => i.account_login)
                .join(", ")}`}
        </span>
      </div>

      <div className="text-sm">
        <span className="text-zinc-500">Webhook URL</span>
        <WebhookUrl url={status.webhook_url} />
      </div>

      {noInstalls && (
        <p className="text-sm text-zinc-500">
          The app isn't installed anywhere yet — install it to pick which
          repositories Projexity can access.
        </p>
      )}

      <div className="flex items-center gap-2 pt-1">
        <a
          href={`${appUrl}/installations/new`}
          target="_blank"
          rel="noreferrer"
          className={noInstalls ? "btn-primary" : "btn-secondary px-3 py-1.5 text-xs"}
        >
          Install / add repositories
        </a>
        {appUrl && (
          <a
            href={appUrl}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1.5 rounded-lg border border-white/[0.06] px-3 py-1.5 text-xs text-zinc-500 transition-all duration-150 hover:border-white/20 hover:text-zinc-300 active:scale-[0.98]"
          >
            Manage on GitHub
          </a>
        )}
      </div>
    </div>
  );
}
