import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Link,
  Outlet,
  useNavigate,
  useRouterState,
} from "@tanstack/react-router";
import { motion, useReducedMotion } from "motion/react";
import gsap from "gsap";
import {
  LayoutGrid,
  LogOut,
  Rocket,
  Server,
  Settings2,
  Shapes,
  type LucideIcon,
} from "lucide-react";
import { api, ApiError } from "../api";
import { Brand } from "../components/Brand";

const nav = [
  { to: "/", label: "Projects", icon: LayoutGrid },
  { to: "/targets", label: "Targets", icon: Server },
  { to: "/apps", label: "Apps", icon: Shapes },
  { to: "/deployments", label: "Deployments", icon: Rocket },
] as const;

function isActive(pathname: string, to: string): boolean {
  if (to === "/") return pathname === "/" || pathname.startsWith("/projects");
  return pathname === to || pathname.startsWith(`${to}/`);
}

function NavItem({
  to,
  label,
  icon: Icon,
  active,
  exact,
}: {
  to: string;
  label: string;
  icon: LucideIcon;
  active: boolean;
  exact?: boolean;
}) {
  return (
    <Link
      to={to}
      data-nav-item
      activeOptions={{ exact }}
      className={`relative flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors duration-150 ${
        active
          ? "bg-white/[0.06] text-zinc-100"
          : "text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-100"
      }`}
    >
      {active && (
        <motion.span
          layoutId="nav-active-bar"
          className="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-emerald-400"
          transition={{ type: "spring", stiffness: 500, damping: 40 }}
        />
      )}
      <Icon className="h-4 w-4" strokeWidth={1.75} />
      {label}
    </Link>
  );
}

export function AppLayout() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const reducedMotion = useReducedMotion();
  const asideRef = useRef<HTMLElement>(null);
  const ready = !!me.data;

  useEffect(() => {
    if (me.error instanceof ApiError && me.error.status === 401) {
      navigate({ to: "/login" });
    }
  }, [me.error, navigate]);

  // One-time sidebar entrance: brand mark, then nav items stagger in.
  useEffect(() => {
    if (!ready || !asideRef.current) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const ctx = gsap.context(() => {
      const tl = gsap.timeline({ defaults: { ease: "power2.out" } });
      tl.from("[data-brand]", { opacity: 0, y: -6, duration: 0.35 })
        .from(
          "[data-nav-item]",
          { opacity: 0, x: -8, duration: 0.25, stagger: 0.03 },
          "-=0.15",
        )
        .from("[data-sidebar-footer]", { opacity: 0, duration: 0.3 }, "-=0.1");
    }, asideRef);
    return () => ctx.revert();
  }, [ready]);

  if (me.isLoading) {
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-3 text-sm text-zinc-500">
        <span className="h-6 w-6 animate-spin rounded-full border-2 border-white/10 border-t-emerald-400" />
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

  const email = me.data.email;
  const initial = (email[0] ?? "?").toUpperCase();

  return (
    <div className="flex h-screen">
      <aside
        ref={asideRef}
        className="flex w-60 shrink-0 flex-col border-r border-white/[0.06] bg-white/[0.02] p-4"
      >
        <div data-brand className="mb-8 px-2 pt-1">
          <Brand withAlpha markClassName="h-[22px] w-[22px]" wordClassName="text-[17px]" />
        </div>

        <p className="mb-2 px-3 text-[10px] font-medium uppercase tracking-widest text-zinc-600">
          Platform
        </p>
        <nav className="flex flex-col gap-1">
          {nav.map((item) => (
            <NavItem
              key={item.to}
              to={item.to}
              label={item.label}
              icon={item.icon}
              active={isActive(pathname, item.to)}
              exact={item.to === "/"}
            />
          ))}
        </nav>

        <nav className="mt-auto flex flex-col gap-1">
          <NavItem
            to="/settings"
            label="Settings"
            icon={Settings2}
            active={isActive(pathname, "/settings")}
          />
        </nav>

        <div
          data-sidebar-footer
          className="mt-4 flex items-center gap-2.5 border-t border-white/[0.06] px-2 pt-4"
        >
          <span
            aria-hidden
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-emerald-500 to-teal-600 text-xs font-semibold text-white"
          >
            {initial}
          </span>
          <p className="min-w-0 flex-1 truncate text-xs text-zinc-500">
            {email}
          </p>
          <button
            onClick={handleLogout}
            title="Sign out"
            className="btn-ghost px-1.5 py-1.5 text-zinc-500"
          >
            <LogOut className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto">
        <motion.div
          key={pathname}
          initial={reducedMotion ? false : { opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.18, ease: "easeOut" }}
          className="mx-auto max-w-6xl px-8 py-10"
        >
          <Outlet />
        </motion.div>
      </main>
    </div>
  );
}
