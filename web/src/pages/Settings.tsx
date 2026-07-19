import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
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
      <h1 className="text-xl font-semibold tracking-tight">Settings</h1>

      {connected && (
        <div className="mt-4 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-4 py-2.5 text-sm text-emerald-300">
          GitHub connected ✓
        </div>
      )}

      <div className="mt-8 max-w-2xl rounded-xl border border-zinc-800 bg-zinc-900/40 p-5">
        <h2 className="font-medium text-zinc-100">GitHub</h2>
        {status.isLoading ? (
          <p className="mt-3 text-sm text-zinc-500">Loading…</p>
        ) : status.isError || !status.data ? (
          <p className="mt-3 text-sm text-red-400">
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
    <div className="mt-3">
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
        className="mt-4 rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
      >
        {create.isPending ? "Redirecting to GitHub…" : "Create GitHub App"}
      </button>
    </div>
  );
}

function GithubConfigured({ status }: { status: GithubAppStatus }) {
  const appUrl = status.app_url ?? "";
  const noInstalls = status.installations.length === 0;
  return (
    <div className="mt-3 space-y-4">
      <div className="flex items-center justify-between text-sm">
        <span className="text-zinc-500">App</span>
        {appUrl ? (
          <a
            href={appUrl}
            target="_blank"
            rel="noreferrer"
            className="font-mono text-emerald-400 hover:underline"
          >
            {status.app_slug}
          </a>
        ) : (
          <span className="font-mono text-zinc-200">{status.app_slug}</span>
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
        <code className="mt-1 block overflow-x-auto rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2 font-mono text-xs text-zinc-300">
          {status.webhook_url}
        </code>
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
          className={
            noInstalls
              ? "rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
              : "rounded-md border border-zinc-700 px-3 py-1.5 text-xs text-zinc-300 transition hover:border-zinc-500"
          }
        >
          Install / add repositories
        </a>
        {appUrl && (
          <a
            href={appUrl}
            target="_blank"
            rel="noreferrer"
            className="rounded-md border border-zinc-800 px-3 py-1.5 text-xs text-zinc-500 transition hover:border-zinc-600 hover:text-zinc-300"
          >
            Manage on GitHub
          </a>
        )}
      </div>
    </div>
  );
}
