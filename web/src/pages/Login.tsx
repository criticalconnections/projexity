import { useEffect, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import gsap from "gsap";
import { api, ApiError } from "../api";
import { Brand } from "../components/Brand";

export function LoginPage() {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const rootRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLFormElement>(null);

  // Ambient orbs drifting slowly + one-time entrance timeline.
  useEffect(() => {
    if (!rootRef.current) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const ctx = gsap.context(() => {
      gsap.to("[data-orb='1']", {
        x: 90,
        y: 60,
        duration: 12,
        yoyo: true,
        repeat: -1,
        ease: "sine.inOut",
      });
      gsap.to("[data-orb='2']", {
        x: -70,
        y: -50,
        duration: 9,
        yoyo: true,
        repeat: -1,
        ease: "sine.inOut",
      });
      gsap.to("[data-orb='3']", {
        x: 50,
        y: -80,
        duration: 14,
        yoyo: true,
        repeat: -1,
        ease: "sine.inOut",
      });

      const tl = gsap.timeline({ defaults: { ease: "power2.out" } });
      tl.from("[data-login-brand]", {
        opacity: 0,
        y: -10,
        scale: 0.92,
        duration: 0.45,
      })
        .from(
          "[data-login-card]",
          { opacity: 0, y: 16, duration: 0.4 },
          "-=0.2",
        )
        .from(
          "[data-login-field]",
          { opacity: 0, y: 8, duration: 0.3, stagger: 0.06 },
          "-=0.2",
        )
        .from("[data-login-footer]", { opacity: 0, duration: 0.35 }, "-=0.15");
    }, rootRef);
    return () => ctx.revert();
  }, []);

  // Small shake on failure.
  useEffect(() => {
    if (!error || !cardRef.current) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const tween = gsap.to(cardRef.current, {
      keyframes: { x: [0, -6, 6, -4, 4, 0] },
      duration: 0.35,
      ease: "power2.out",
    });
    return () => {
      tween.kill();
    };
  }, [error]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const fn = mode === "login" ? api.login : api.register;
      await fn(email, password);
      await queryClient.invalidateQueries({ queryKey: ["me"] });
      navigate({ to: "/" });
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Something went wrong");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      ref={rootRef}
      className="relative flex min-h-screen items-center justify-center overflow-hidden px-4"
    >
      {/* ambient background */}
      <div aria-hidden className="pointer-events-none absolute inset-0">
        <div
          data-orb="1"
          className="absolute -top-32 left-1/4 h-[420px] w-[420px] rounded-full bg-emerald-500/[0.05] blur-3xl"
        />
        <div
          data-orb="2"
          className="absolute bottom-[-140px] right-[15%] h-[380px] w-[380px] rounded-full bg-teal-500/[0.05] blur-3xl"
        />
        <div
          data-orb="3"
          className="absolute left-[-120px] top-1/2 h-[320px] w-[320px] rounded-full bg-emerald-400/[0.04] blur-3xl"
        />
      </div>

      <div className="relative w-full max-w-sm">
        <div data-login-brand className="mb-8 flex flex-col items-center">
          <Brand markClassName="h-8 w-8" wordClassName="text-2xl" />
          <p className="mt-3 text-sm text-zinc-400">
            Deploy to your own infrastructure.
          </p>
        </div>

        <form
          ref={cardRef}
          data-login-card
          onSubmit={handleSubmit}
          className="card border-white/[0.08] bg-white/[0.03] p-6 backdrop-blur"
        >
          <label
            data-login-field
            className="block text-[11px] font-medium uppercase tracking-wider text-zinc-500"
          >
            Email
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="input mt-1.5 font-sans normal-case tracking-normal"
            />
          </label>
          <label
            data-login-field
            className="mt-4 block text-[11px] font-medium uppercase tracking-wider text-zinc-500"
          >
            Password
            <input
              type="password"
              required
              minLength={8}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="input mt-1.5 font-sans normal-case tracking-normal"
            />
          </label>
          {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
          <button
            data-login-field
            type="submit"
            disabled={busy}
            className="btn-primary mt-6 w-full"
          >
            {busy
              ? "Working…"
              : mode === "login"
                ? "Sign in"
                : "Create account"}
          </button>
        </form>

        <p data-login-footer className="mt-4 text-center text-sm text-zinc-500">
          {mode === "login" ? (
            <>
              New here?{" "}
              <button
                className="text-emerald-400 transition-colors hover:text-emerald-300 hover:underline"
                onClick={() => setMode("register")}
              >
                Create an account
              </button>
            </>
          ) : (
            <>
              Already have an account?{" "}
              <button
                className="text-emerald-400 transition-colors hover:text-emerald-300 hover:underline"
                onClick={() => setMode("login")}
              >
                Sign in
              </button>
            </>
          )}
        </p>
        <p
          data-login-footer
          className="mt-8 text-center text-xs text-zinc-600"
        >
          Deploy to infrastructure you own.
        </p>
      </div>
    </div>
  );
}
