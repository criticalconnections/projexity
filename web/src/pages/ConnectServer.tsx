import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import {
  api,
  ApiError,
  parseBootstrapSteps,
  type CheckResponse,
  type ServerFacts,
  type Target,
} from "../api";

type Step = "intro" | "name" | "host" | "key" | "check" | "bootstrap" | "done";

const STEP_ORDER: Step[] = ["intro", "name", "host", "key", "check", "bootstrap", "done"];

/** Typeform-style full-screen connect-a-server wizard: one question at a
 * time, big type, Enter to continue. */
export function ConnectServerPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const [step, setStep] = useState<Step>("intro");
  const [direction, setDirection] = useState(1);
  const [name, setName] = useState("");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [sshUser, setSshUser] = useState("root");
  const [target, setTarget] = useState<Target | null>(null);
  const [check, setCheck] = useState<CheckResponse | null>(null);
  const [checking, setChecking] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const go = useCallback((next: Step) => {
    setDirection(STEP_ORDER.indexOf(next) >= STEP_ORDER.indexOf(step) ? 1 : -1);
    setFormError(null);
    setStep(next);
  }, [step]);

  const close = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["targets"] });
    navigate({ to: "/targets" });
  }, [navigate, queryClient]);

  // Esc closes (except mid-bootstrap, where leaving is fine too — the job
  // keeps running server-side, the targets list shows progress).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const createTarget = useMutation({
    mutationFn: async () => {
      // Re-doing the host step replaces the previously created target so
      // each attempt gets a fresh key bound to the right host.
      if (target) await api.deleteTarget(target.id).catch(() => {});
      return api.createTarget({
        name: name.trim(),
        host: host.trim(),
        port: parseInt(port, 10) || 22,
        ssh_user: sshUser.trim() || "root",
      });
    },
    onSuccess: (t) => {
      setTarget(t);
      setCheck(null);
      go("key");
    },
    onError: (e) =>
      setFormError(e instanceof ApiError ? e.message : "Something went wrong"),
  });

  const runCheck = useCallback(async () => {
    if (!target) return;
    setChecking(true);
    setCheck(null);
    go("check");
    try {
      setCheck(await api.checkTarget(target.id));
    } catch (e) {
      setCheck({
        ok: false,
        facts: null,
        issues: [],
        error: e instanceof ApiError ? e.message : "Something went wrong",
      });
    } finally {
      setChecking(false);
    }
  }, [target, go]);

  const startBootstrap = useMutation({
    mutationFn: () => api.bootstrapTarget(target!.id),
    onSuccess: () => go("bootstrap"),
    onError: (e) =>
      setFormError(e instanceof ApiError ? e.message : "Something went wrong"),
  });

  const stepIndex = STEP_ORDER.indexOf(step);
  const progress = (stepIndex / (STEP_ORDER.length - 1)) * 100;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-zinc-950">
      {/* progress bar */}
      <div className="h-1 w-full bg-zinc-900">
        <motion.div
          className="h-full bg-emerald-500"
          animate={{ width: `${progress}%` }}
          transition={{ type: "spring", stiffness: 120, damping: 20 }}
        />
      </div>

      {/* header */}
      <div className="flex items-center justify-between px-6 py-4">
        {stepIndex > 0 && stepIndex < 5 ? (
          <button
            onClick={() => go(STEP_ORDER[stepIndex - 1])}
            className="rounded-md px-2 py-1 text-sm text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200"
          >
            ← Back
          </button>
        ) : (
          <span />
        )}
        <button
          onClick={close}
          className="rounded-md px-2 py-1 text-sm text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200"
        >
          ✕
        </button>
      </div>

      {/* step body */}
      <div className="flex flex-1 items-center justify-center overflow-y-auto px-6 pb-16">
        <div className="w-full max-w-2xl">
          <AnimatePresence mode="wait" custom={direction}>
            <motion.div
              key={step}
              custom={direction}
              initial={{ opacity: 0, y: direction > 0 ? 40 : -40 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: direction > 0 ? -40 : 40 }}
              transition={{ duration: 0.28, ease: "easeOut" }}
            >
              {step === "intro" && <Intro onNext={() => go("name")} />}

              {step === "name" && (
                <QuestionForm
                  n={1}
                  question="What should we call this server?"
                  hint="Just a label — you can change it later."
                  onSubmit={() => name.trim() && go("host")}
                >
                  <BigInput
                    value={name}
                    onChange={setName}
                    placeholder="production-1"
                    autoFocus
                  />
                  <NextButton disabled={!name.trim()} />
                </QuestionForm>
              )}

              {step === "host" && (
                <QuestionForm
                  n={2}
                  question="Where do we reach it?"
                  hint="The public IP address or hostname of your server."
                  onSubmit={() => host.trim() && createTarget.mutate()}
                >
                  <BigInput
                    value={host}
                    onChange={setHost}
                    placeholder="203.0.113.7"
                    autoFocus
                  />
                  <div className="mt-6 flex gap-6">
                    <label className="text-sm text-zinc-500">
                      SSH port
                      <input
                        value={port}
                        onChange={(e) => setPort(e.target.value)}
                        inputMode="numeric"
                        className="mt-1 block w-24 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-200 outline-none focus:border-emerald-500"
                      />
                    </label>
                    <label className="text-sm text-zinc-500">
                      SSH user
                      <input
                        value={sshUser}
                        onChange={(e) => setSshUser(e.target.value)}
                        className="mt-1 block w-36 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-200 outline-none focus:border-emerald-500"
                      />
                      <span className="mt-1 block text-xs text-zinc-600">
                        root, or a user with passwordless sudo
                      </span>
                    </label>
                  </div>
                  {formError && <ErrorText message={formError} />}
                  <NextButton
                    disabled={!host.trim()}
                    busy={createTarget.isPending}
                  />
                </QuestionForm>
              )}

              {step === "key" && target && (
                <KeyStep target={target} onNext={runCheck} />
              )}

              {step === "check" && (
                <CheckStep
                  checking={checking}
                  check={check}
                  onRetry={runCheck}
                  onBack={() => go("host")}
                  busy={startBootstrap.isPending}
                  formError={formError}
                  onContinue={() => startBootstrap.mutate()}
                />
              )}

              {step === "bootstrap" && target && (
                <BootstrapStepView
                  targetId={target.id}
                  onReady={() => go("done")}
                  onRetry={() => startBootstrap.mutate()}
                />
              )}

              {step === "done" && target && (
                <DoneStep name={target.name} onClose={close} />
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}

/* ---------- building blocks ---------- */

function QuestionForm({
  n,
  question,
  hint,
  children,
  onSubmit,
}: {
  n: number;
  question: string;
  hint?: string;
  children: React.ReactNode;
  onSubmit: () => void;
}) {
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      <p className="mb-2 text-sm font-medium text-emerald-400">{n} →</p>
      <h1 className="text-3xl font-semibold tracking-tight text-zinc-100">
        {question}
      </h1>
      {hint && <p className="mt-2 text-zinc-500">{hint}</p>}
      <div className="mt-8">{children}</div>
    </form>
  );
}

function BigInput({
  value,
  onChange,
  placeholder,
  autoFocus,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
  autoFocus?: boolean;
}) {
  return (
    <input
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      autoFocus={autoFocus}
      spellCheck={false}
      autoComplete="off"
      className="w-full border-b-2 border-zinc-800 bg-transparent pb-2 text-3xl text-zinc-100 outline-none transition-colors placeholder:text-zinc-700 focus:border-emerald-500"
    />
  );
}

function NextButton({ disabled, busy }: { disabled?: boolean; busy?: boolean }) {
  return (
    <div className="mt-8 flex items-center gap-3">
      <button
        type="submit"
        disabled={disabled || busy}
        className="rounded-lg bg-emerald-600 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
      >
        {busy ? "Working…" : "Continue"}
      </button>
      <span className="text-xs text-zinc-600">
        press <kbd className="rounded border border-zinc-700 px-1">Enter ↵</kbd>
      </span>
    </div>
  );
}

function ErrorText({ message }: { message: string }) {
  return <p className="mt-4 text-sm text-red-400">{message}</p>;
}

/* ---------- steps ---------- */

function Intro({ onNext }: { onNext: () => void }) {
  return (
    <div>
      <h1 className="text-4xl font-semibold tracking-tight text-zinc-100">
        Connect a server
      </h1>
      <p className="mt-3 max-w-lg text-lg text-zinc-400">
        Any Linux VPS with SSH works — Hetzner, DigitalOcean, a home lab. In
        about two minutes it'll be ready to run your apps with automatic
        HTTPS.
      </p>
      <ul className="mt-8 space-y-3 text-zinc-400">
        {[
          "You add our SSH key to the server (one command)",
          "We check the server and tell you about anything odd",
          "We install Docker and a Caddy proxy — nothing else",
        ].map((line, i) => (
          <motion.li
            key={line}
            initial={{ opacity: 0, x: -12 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 0.15 + i * 0.12 }}
            className="flex items-start gap-3"
          >
            <span className="mt-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-emerald-500/10 text-xs text-emerald-400">
              {i + 1}
            </span>
            {line}
          </motion.li>
        ))}
      </ul>
      <div className="mt-10 flex items-center gap-4">
        <button
          onClick={onNext}
          autoFocus
          className="rounded-lg bg-emerald-600 px-6 py-3 font-medium text-white transition hover:bg-emerald-500"
        >
          Let's go
        </button>
        <span className="rounded-md border border-zinc-800 px-3 py-1.5 text-sm text-zinc-500">
          Kubernetes cluster — soon
        </span>
      </div>
    </div>
  );
}

function KeyStep({ target, onNext }: { target: Target; onNext: () => void }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(target.setup_command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return (
    <div>
      <p className="mb-2 text-sm font-medium text-emerald-400">3 →</p>
      <h1 className="text-3xl font-semibold tracking-tight text-zinc-100">
        Authorize Projexity's key
      </h1>
      <p className="mt-2 max-w-lg text-zinc-500">
        We generated a fresh SSH key just for this server — the private half
        never leaves your Projexity instance. Run this on{" "}
        <span className="text-zinc-300">
          {target.ssh_user}@{target.host}
        </span>
        :
      </p>
      <div className="group relative mt-6">
        <pre className="overflow-x-auto rounded-xl border border-zinc-800 bg-zinc-900/70 p-4 pr-20 text-sm text-emerald-300">
          {target.setup_command}
        </pre>
        <button
          onClick={copy}
          className="absolute right-3 top-3 rounded-md border border-zinc-700 bg-zinc-900 px-2.5 py-1 text-xs text-zinc-300 transition hover:border-zinc-500"
        >
          {copied ? "Copied ✓" : "Copy"}
        </button>
      </div>
      <p className="mt-3 text-xs text-zinc-600">
        Tip: already in a terminal? <code>ssh {target.ssh_user}@{target.host}</code>{" "}
        and paste it there.
      </p>
      <div className="mt-8">
        <button
          onClick={onNext}
          className="rounded-lg bg-emerald-600 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-emerald-500"
        >
          I've added it — test the connection
        </button>
      </div>
    </div>
  );
}

function CheckStep({
  checking,
  check,
  onRetry,
  onBack,
  onContinue,
  busy,
  formError,
}: {
  checking: boolean;
  check: CheckResponse | null;
  onRetry: () => void;
  onBack: () => void;
  onContinue: () => void;
  busy: boolean;
  formError: string | null;
}) {
  if (checking || !check) {
    return (
      <div className="text-center">
        <Spinner large />
        <h1 className="mt-6 text-2xl font-semibold text-zinc-100">
          Knocking on the door…
        </h1>
        <p className="mt-2 text-zinc-500">
          Connecting over SSH and looking around. This takes a few seconds.
        </p>
      </div>
    );
  }

  if (!check.ok) {
    return (
      <div>
        <p className="mb-2 text-sm font-medium text-red-400">
          Couldn't connect
        </p>
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-100">
          {check.error}
        </h1>
        <p className="mt-3 max-w-lg text-zinc-500">
          Double-check that you ran the key command on the right server, then
          try again.
        </p>
        <div className="mt-8 flex gap-3">
          <button
            onClick={onRetry}
            className="rounded-lg bg-emerald-600 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-emerald-500"
          >
            Try again
          </button>
          <button
            onClick={onBack}
            className="rounded-lg border border-zinc-700 px-5 py-2.5 text-sm text-zinc-300 transition hover:border-zinc-500"
          >
            Edit connection details
          </button>
        </div>
      </div>
    );
  }

  const f = check.facts!;
  const blocked = check.issues.some((i) => i.severity === "error");
  const rows = factRows(f);

  return (
    <div>
      <p className="mb-2 text-sm font-medium text-emerald-400">Connected ✓</p>
      <h1 className="text-3xl font-semibold tracking-tight text-zinc-100">
        Here's what we found
      </h1>
      <div className="mt-6 space-y-2">
        {rows.map((row, i) => (
          <motion.div
            key={row.label}
            initial={{ opacity: 0, x: -10 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: i * 0.07 }}
            className="flex items-center justify-between rounded-lg border border-zinc-800/70 bg-zinc-900/40 px-4 py-2.5 text-sm"
          >
            <span className="text-zinc-500">{row.label}</span>
            <span className={row.ok ? "text-zinc-200" : "text-amber-400"}>
              {row.value}
            </span>
          </motion.div>
        ))}
      </div>
      {check.issues.length > 0 && (
        <div className="mt-4 space-y-2">
          {check.issues.map((issue) => (
            <div
              key={issue.message}
              className={`rounded-lg border px-4 py-2.5 text-sm ${
                issue.severity === "error"
                  ? "border-red-500/30 bg-red-500/5 text-red-300"
                  : issue.severity === "warning"
                    ? "border-amber-500/30 bg-amber-500/5 text-amber-300"
                    : "border-zinc-800 bg-zinc-900/40 text-zinc-400"
              }`}
            >
              {issue.message}
            </div>
          ))}
        </div>
      )}
      {formError && <ErrorText message={formError} />}
      <div className="mt-8 flex items-center gap-3">
        <button
          onClick={onContinue}
          disabled={blocked || busy}
          className="rounded-lg bg-emerald-600 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
        >
          {busy ? "Starting…" : "Set up this server"}
        </button>
        <button
          onClick={onRetry}
          className="rounded-lg border border-zinc-700 px-5 py-2.5 text-sm text-zinc-300 transition hover:border-zinc-500"
        >
          Re-check
        </button>
      </div>
      {blocked && (
        <p className="mt-3 text-sm text-zinc-500">
          Fix the items marked in red, then re-check.
        </p>
      )}
    </div>
  );
}

function BootstrapStepView({
  targetId,
  onReady,
  onRetry,
}: {
  targetId: string;
  onReady: () => void;
  onRetry: () => void;
}) {
  const firedRef = useRef(false);
  const { data: target } = useQuery({
    queryKey: ["target", targetId],
    queryFn: () => api.getTarget(targetId),
    refetchInterval: (q) =>
      q.state.data?.status === "bootstrapping" || !q.state.data ? 1200 : false,
  });

  useEffect(() => {
    if (target?.status === "ready" && !firedRef.current) {
      firedRef.current = true;
      // Small beat so the last ✓ lands before we celebrate.
      setTimeout(onReady, 700);
    }
  }, [target?.status, onReady]);

  const steps = target ? parseBootstrapSteps(target.status_detail) : [];
  const failed = target?.status === "error";

  return (
    <div>
      <p className="mb-2 text-sm font-medium text-emerald-400">
        {failed ? "Setup hit a snag" : "Setting up your server"}
      </p>
      <h1 className="text-3xl font-semibold tracking-tight text-zinc-100">
        {failed ? "Something went wrong" : "Sit back for a minute…"}
      </h1>
      <div className="mt-8 space-y-3">
        {steps.map((s) => (
          <div key={s.id} className="flex items-start gap-3">
            <StepIcon status={s.status} />
            <div className="min-w-0">
              <p
                className={`text-sm ${
                  s.status === "pending" ? "text-zinc-600" : "text-zinc-200"
                }`}
              >
                {s.label}
                {s.status === "skipped" && (
                  <span className="ml-2 text-xs text-zinc-500">
                    already done
                  </span>
                )}
              </p>
              {s.detail && (
                <p
                  className={`mt-0.5 truncate text-xs ${
                    s.status === "failed" ? "text-red-400" : "text-zinc-500"
                  }`}
                >
                  {s.detail}
                </p>
              )}
            </div>
          </div>
        ))}
      </div>
      {failed && (
        <div className="mt-8 flex gap-3">
          <button
            onClick={onRetry}
            className="rounded-lg bg-emerald-600 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-emerald-500"
          >
            Retry setup
          </button>
          <p className="self-center text-sm text-zinc-500">
            Setup is safe to re-run — finished steps are skipped.
          </p>
        </div>
      )}
    </div>
  );
}

function DoneStep({ name, onClose }: { name: string; onClose: () => void }) {
  return (
    <div className="text-center">
      <motion.div
        initial={{ scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 180, damping: 12 }}
        className="mx-auto flex h-20 w-20 items-center justify-center rounded-full bg-emerald-500/15 text-4xl text-emerald-400 shadow-[0_0_60px_-10px] shadow-emerald-500/40"
      >
        ✓
      </motion.div>
      <h1 className="mt-6 text-4xl font-semibold tracking-tight text-zinc-100">
        {name} is ready
      </h1>
      <p className="mx-auto mt-3 max-w-md text-zinc-400">
        Docker is running and the proxy is live. Point a wildcard DNS record at
        this server and every app you deploy gets an instant HTTPS URL.
      </p>
      <button
        onClick={onClose}
        autoFocus
        className="mt-10 rounded-lg bg-emerald-600 px-6 py-3 font-medium text-white transition hover:bg-emerald-500"
      >
        Take me to my servers
      </button>
    </div>
  );
}

/* ---------- bits ---------- */

function Spinner({ large }: { large?: boolean }) {
  const size = large ? "h-10 w-10 border-[3px]" : "h-4 w-4 border-2";
  return (
    <div
      className={`${size} mx-auto animate-spin rounded-full border-zinc-700 border-t-emerald-400`}
    />
  );
}

function StepIcon({ status }: { status: string }) {
  if (status === "done" || status === "skipped")
    return (
      <span className="flex h-5 w-5 items-center justify-center rounded-full bg-emerald-500/15 text-xs text-emerald-400">
        ✓
      </span>
    );
  if (status === "failed")
    return (
      <span className="flex h-5 w-5 items-center justify-center rounded-full bg-red-500/15 text-xs text-red-400">
        ✕
      </span>
    );
  if (status === "running")
    return (
      <span className="flex h-5 w-5 items-center justify-center">
        <span className="h-4 w-4 animate-spin rounded-full border-2 border-zinc-700 border-t-emerald-400" />
      </span>
    );
  return (
    <span className="flex h-5 w-5 items-center justify-center text-zinc-700">
      ○
    </span>
  );
}

function factRows(f: ServerFacts) {
  const gib = (b: number | null) =>
    b == null ? "unknown" : `${(b / 1024 ** 3).toFixed(1)} GiB`;
  return [
    {
      label: "Operating system",
      value: f.distro ?? f.os,
      ok: f.os === "Linux",
    },
    { label: "Architecture", value: f.arch, ok: true },
    {
      label: "Access",
      value:
        f.access === "root"
          ? "root"
          : f.access === "sudo"
            ? "passwordless sudo"
            : "insufficient",
      ok: f.access !== "none",
    },
    {
      label: "Docker",
      value: f.docker_version ?? "not installed — we'll handle it",
      ok: true,
    },
    {
      label: "Ports 80 / 443",
      value:
        f.caddy_running
          ? "held by Projexity's proxy"
          : f.port_80_free && f.port_443_free
            ? "free"
            : "in use",
      ok: f.caddy_running || (f.port_80_free && f.port_443_free),
    },
    { label: "Free disk", value: gib(f.disk_free_bytes), ok: true },
    { label: "Memory", value: gib(f.memory_total_bytes), ok: true },
  ];
}
