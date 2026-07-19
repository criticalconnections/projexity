import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, Outlet, useNavigate } from "@tanstack/react-router";
import { api, ApiError } from "../api";

const nav = [
  { to: "/", label: "Projects" },
  { to: "/targets", label: "Targets" },
  { to: "/deployments", label: "Deployments" },
] as const;

export function AppLayout() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });

  useEffect(() => {
    if (me.error instanceof ApiError && me.error.status === 401) {
      navigate({ to: "/login" });
    }
  }, [me.error, navigate]);

  if (me.isLoading) {
    return (
      <div className="flex h-screen items-center justify-center text-zinc-500">
        Loading…
      </div>
    );
  }
  if (!me.data) return null;

  async function handleLogout() {
    await api.logout();
    queryClient.clear();
    navigate({ to: "/login" });
  }

  return (
    <div className="flex h-screen">
      <aside className="flex w-56 flex-col border-r border-zinc-800 bg-zinc-900/40 p-4">
        <div className="mb-8 flex items-center gap-2 px-2">
          <span className="text-lg font-semibold tracking-tight">
            projexity
          </span>
          <span className="rounded bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-medium text-emerald-400">
            alpha
          </span>
        </div>
        <nav className="flex flex-col gap-1">
          {nav.map((item) => (
            <Link
              key={item.to}
              to={item.to}
              className="rounded-md px-3 py-2 text-sm text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100"
              activeProps={{
                className:
                  "rounded-md px-3 py-2 text-sm bg-zinc-800 text-zinc-100",
              }}
              activeOptions={{ exact: item.to === "/" }}
            >
              {item.label}
            </Link>
          ))}
        </nav>
        <div className="mt-auto border-t border-zinc-800 pt-4">
          <p className="truncate px-2 text-xs text-zinc-500">{me.data.email}</p>
          <button
            onClick={handleLogout}
            className="mt-2 w-full rounded-md px-3 py-2 text-left text-sm text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100"
          >
            Sign out
          </button>
        </div>
      </aside>
      <main className="flex-1 overflow-y-auto p-8">
        <Outlet />
      </main>
    </div>
  );
}
