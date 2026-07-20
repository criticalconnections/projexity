import { useCallback, useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  ArrowLeft,
  Boxes,
  Check,
  ChevronRight,
  Cpu,
  FileKey,
  Globe,
  Network,
  ShieldCheck,
  X,
  type LucideIcon,
} from "lucide-react";
import { api, ApiError, type Target } from "../api";

type Step = "intro" | "name" | "config" | "done";

const STEP_ORDER: Step[] = ["intro", "name", "config", "done"];

/** Typeform-style full-screen connect-a-cluster wizard: name it, paste a
 * kubeconfig, we validate against the cluster synchronously. */
export function ConnectClusterPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const reducedMotion = useReducedMotion();

  const [step, setStep] = useState<Step>("intro");
  const [direction, setDirection] = useState(1);
  const [name, setName] = useState("");
  const [kubeconfig, setKubeconfig] = useState("");
  const [namespace, setNamespace] = useState("projexity");
  const [ingressClass, setIngressClass] = useState("");
  const [domainBase, setDomainBase] = useState("");
  const [clusterIssuer, setClusterIssuer] = useState("");
  const [target, setTarget] = useState<Target | null>(null);
  const [formError, setFormError] = useState<string | null>(null);

  const go = useCallback(
    (next: Step) => {
      setDirection(STEP_ORDER.indexOf(next) >= STEP_ORDER.indexOf(step) ? 1 : -1);
      setFormError(null);
      setStep(next);
    },
    [step],
  );

  const close = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["targets"] });
    navigate({ to: "/targets" });
  }, [navigate, queryClient]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const connect = useMutation({
    mutationFn: () =>
      api.createCluster({
        name: name.trim(),
        kubeconfig,
        namespace: namespace.trim() || undefined,
        ingress_class: ingressClass.trim() || undefined,
        domain_base: domainBase.trim() || undefined,
        cluster_issuer: clusterIssuer.trim() || undefined,
      }),
    onSuccess: (t) => {
      setTarget(t);
      go("done");
    },
    onError: (e) =>
      setFormError(e instanceof ApiError ? e.message : "Something went wrong"),
  });

  const stepIndex = STEP_ORDER.indexOf(step);
  const progress = (stepIndex / (STEP_ORDER.length - 1)) * 100;
  const counter = `${String(stepIndex + 1).padStart(2, "0")} / ${String(
    STEP_ORDER.length,
  ).padStart(2, "0")}`;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-canvas">
      {/* progress bar */}
      <div className="h-0.5 w-full bg-white/[0.06]">
        <motion.div
          className="h-full rounded-r-full bg-gradient-to-r from-emerald-500 to-teal-400 shadow-[0_0_12px_rgba(16,185,129,0.5)]"
          animate={{ width: `${progress}%` }}
          transition={
            reducedMotion
              ? { duration: 0 }
              : { type: "spring", stiffness: 120, damping: 20 }
          }
        />
      </div>

      {/* header */}
      <div className="flex items-center justify-between px-6 py-4">
        <div className="flex items-center gap-3">
          <span className="font-mono text-xs text-zinc-600">{counter}</span>
          {stepIndex > 0 && stepIndex < 3 && (
            <button
              onClick={() => go(STEP_ORDER[stepIndex - 1])}
              className="btn-ghost px-2 py-1 text-sm"
            >
              <ArrowLeft className="h-4 w-4" strokeWidth={1.75} />
              Back
            </button>
          )}
        </div>
        <button onClick={close} title="Close" className="btn-ghost px-2 py-1.5">
          <X className="h-4 w-4" strokeWidth={1.75} />
        </button>
      </div>

      {/* step body */}
      <div className="flex flex-1 items-center justify-center overflow-y-auto px-6 pb-16">
        <div className="w-full max-w-2xl">
          <AnimatePresence mode="wait" custom={direction}>
            <motion.div
              key={step}
              custom={direction}
              initial={
                reducedMotion ? false : { opacity: 0, y: direction > 0 ? 40 : -40 }
              }
              animate={{ opacity: 1, y: 0 }}
              exit={
                reducedMotion
                  ? { opacity: 0 }
                  : { opacity: 0, y: direction > 0 ? -40 : 40 }
              }
              transition={{ duration: reducedMotion ? 0 : 0.28, ease: "easeOut" }}
            >
              {step === "intro" && (
                <Intro
                  reducedMotion={reducedMotion}
                  onNext={() => go("name")}
                />
              )}

              {step === "name" && (
                <QuestionForm
                  n={1}
                  question="What should we call this cluster?"
                  hint="Just a label — you can change it later."
                  onSubmit={() => name.trim() && go("config")}
                >
                  <BigInput
                    value={name}
                    onChange={setName}
                    placeholder="prod-cluster"
                    autoFocus
                  />
                  <NextButton disabled={!name.trim()} />
                </QuestionForm>
              )}

              {step === "config" && (
                <ConfigStep
                  kubeconfig={kubeconfig}
                  setKubeconfig={setKubeconfig}
                  namespace={namespace}
                  setNamespace={setNamespace}
                  ingressClass={ingressClass}
                  setIngressClass={setIngressClass}
                  domainBase={domainBase}
                  setDomainBase={setDomainBase}
                  clusterIssuer={clusterIssuer}
                  setClusterIssuer={setClusterIssuer}
                  busy={connect.isPending}
                  formError={formError}
                  onSubmit={() => {
                    if (kubeconfig.trim() && !connect.isPending) {
                      setFormError(null);
                      connect.mutate();
                    }
                  }}
                />
              )}

              {step === "done" && target && (
                <DoneStep
                  target={target}
                  reducedMotion={reducedMotion}
                  onClose={close}
                />
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
      <p className="mb-3 font-mono text-xs font-medium tracking-wider text-emerald-400">
        0{n} →
      </p>
      <h1 className="text-4xl font-semibold tracking-tight text-zinc-100">
        {question}
      </h1>
      {hint && <p className="mt-3 text-zinc-500">{hint}</p>}
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
    <div>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoFocus={autoFocus}
        spellCheck={false}
        autoComplete="off"
        className="peer w-full border-b border-white/10 bg-transparent pb-2 text-4xl tracking-tight text-zinc-100 caret-emerald-400 outline-none transition-colors placeholder:text-zinc-700 focus-visible:shadow-none"
      />
      <span
        aria-hidden
        className="block h-px origin-left scale-x-0 bg-gradient-to-r from-emerald-400 to-teal-400 transition-transform duration-300 ease-out peer-focus:scale-x-100"
      />
    </div>
  );
}

function NextButton({ disabled, busy }: { disabled?: boolean; busy?: boolean }) {
  return (
    <div className="mt-8 flex items-center gap-3">
      <button type="submit" disabled={disabled || busy} className="btn-primary px-5 py-2.5">
        {busy ? "Working…" : "Continue"}
      </button>
      <span className="text-xs text-zinc-600">
        press <kbd className="kbd">Enter ↵</kbd>
      </span>
    </div>
  );
}

function ErrorText({ message }: { message: string }) {
  return <p className="mt-4 text-sm text-red-400">{message}</p>;
}

/* ---------- steps ---------- */

const INTRO_POINTS: { icon: LucideIcon; text: string }[] = [
  { icon: FileKey, text: "You paste a kubeconfig — we validate it against the cluster" },
  {
    icon: Network,
    text: "Apps deploy into one namespace and get Ingress routes",
  },
  {
    icon: ShieldCheck,
    text: "Nothing is installed cluster-wide — remove the namespace, we're gone",
  },
];

function Intro({
  reducedMotion,
  onNext,
}: {
  reducedMotion: boolean | null;
  onNext: () => void;
}) {
  return (
    <div>
      <h1 className="text-4xl font-semibold tracking-tight text-zinc-100">
        Connect a Kubernetes cluster
      </h1>
      <p className="mt-3 max-w-lg text-lg text-zinc-400">
        Point Projexity at an existing cluster — k3s, EKS, GKE, anything that
        speaks the Kubernetes API — and deploy apps onto it like any other
        target.
      </p>
      <ul className="mt-8 space-y-3 text-zinc-400">
        {INTRO_POINTS.map(({ icon: Icon, text }, i) => (
          <motion.li
            key={text}
            initial={reducedMotion ? false : { opacity: 0, x: -12 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: reducedMotion ? 0 : 0.15 + i * 0.12 }}
            className="flex items-start gap-3"
          >
            <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-lg border border-emerald-500/20 bg-emerald-500/10 text-emerald-400">
              <Icon className="h-3.5 w-3.5" strokeWidth={1.75} />
            </span>
            {text}
          </motion.li>
        ))}
      </ul>
      <p className="mt-6 max-w-lg text-sm text-zinc-600">
        You'll need a kubeconfig with a token or client-certificate
        (exec-plugin auth like EKS/GKE{" "}
        <code className="font-mono text-zinc-500">aws</code> /{" "}
        <code className="font-mono text-zinc-500">gcloud</code> isn't supported
        — mint a ServiceAccount token instead).
      </p>
      <div className="mt-8">
        <button onClick={onNext} autoFocus className="btn-primary px-6 py-3">
          Let's go
        </button>
      </div>
    </div>
  );
}

function ConfigStep({
  kubeconfig,
  setKubeconfig,
  namespace,
  setNamespace,
  ingressClass,
  setIngressClass,
  domainBase,
  setDomainBase,
  clusterIssuer,
  setClusterIssuer,
  busy,
  formError,
  onSubmit,
}: {
  kubeconfig: string;
  setKubeconfig: (v: string) => void;
  namespace: string;
  setNamespace: (v: string) => void;
  ingressClass: string;
  setIngressClass: (v: string) => void;
  domainBase: string;
  setDomainBase: (v: string) => void;
  clusterIssuer: string;
  setClusterIssuer: (v: string) => void;
  busy: boolean;
  formError: string | null;
  onSubmit: () => void;
}) {
  const [showAdvanced, setShowAdvanced] = useState(false);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      <p className="mb-3 font-mono text-xs font-medium tracking-wider text-emerald-400">
        02 →
      </p>
      <h1 className="text-3xl font-semibold tracking-tight text-zinc-100">
        Paste your kubeconfig
      </h1>
      <p className="mt-3 max-w-lg text-zinc-500">
        We connect to the cluster right away to make sure it works. The
        credentials are stored encrypted on your Projexity instance.
      </p>
      <textarea
        value={kubeconfig}
        onChange={(e) => setKubeconfig(e.target.value)}
        rows={12}
        spellCheck={false}
        autoComplete="off"
        autoFocus
        placeholder="Paste your kubeconfig YAML here…"
        className="input mt-6 resize-y font-mono text-[13px] leading-relaxed"
      />

      <button
        type="button"
        onClick={() => setShowAdvanced((v) => !v)}
        className="btn-ghost mt-4 px-2 py-1 text-sm"
      >
        <ChevronRight
          className={`h-4 w-4 transition-transform duration-150 ${
            showAdvanced ? "rotate-90" : ""
          }`}
          strokeWidth={1.75}
        />
        Advanced
      </button>
      {showAdvanced && (
        <div className="mt-3 grid gap-4 sm:grid-cols-2">
          <label className="text-sm text-zinc-500">
            Namespace
            <input
              value={namespace}
              onChange={(e) => setNamespace(e.target.value)}
              spellCheck={false}
              className="input mt-1 block font-mono"
            />
            <span className="mt-1 block text-xs text-zinc-600">
              Everything Projexity deploys lives here
            </span>
          </label>
          <label className="text-sm text-zinc-500">
            Ingress class
            <input
              value={ingressClass}
              onChange={(e) => setIngressClass(e.target.value)}
              spellCheck={false}
              placeholder="traefik / nginx — leave blank for cluster default"
              className="input mt-1 block font-mono"
            />
          </label>
          <label className="text-sm text-zinc-500">
            Domain base
            <input
              value={domainBase}
              onChange={(e) => setDomainBase(e.target.value)}
              spellCheck={false}
              placeholder="apps.example.com — apps get <name>.<base>"
              className="input mt-1 block font-mono"
            />
          </label>
          <label className="text-sm text-zinc-500">
            cert-manager cluster issuer
            <input
              value={clusterIssuer}
              onChange={(e) => setClusterIssuer(e.target.value)}
              spellCheck={false}
              placeholder="letsencrypt-prod (optional)"
              className="input mt-1 block font-mono"
            />
          </label>
        </div>
      )}

      {formError && <ErrorText message={formError} />}
      <div className="mt-8 flex items-center gap-3">
        <button
          type="submit"
          disabled={!kubeconfig.trim() || busy}
          className="btn-primary px-5 py-2.5"
        >
          {busy ? "Validating cluster…" : "Connect cluster"}
        </button>
        {busy && (
          <span className="text-xs text-zinc-600">
            Talking to the API server — a few seconds…
          </span>
        )}
      </div>
    </form>
  );
}

function DoneStep({
  target,
  reducedMotion,
  onClose,
}: {
  target: Target;
  reducedMotion: boolean | null;
  onClose: () => void;
}) {
  const info = target.cluster?.info ?? null;
  return (
    <div className="text-center">
      <motion.div
        initial={reducedMotion ? false : { scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={
          reducedMotion
            ? { duration: 0 }
            : { type: "spring", stiffness: 180, damping: 12 }
        }
        className="mx-auto flex h-20 w-20 items-center justify-center rounded-full border border-emerald-500/25 bg-emerald-500/15 text-emerald-400 shadow-[0_0_60px_-10px] shadow-emerald-500/40"
      >
        <Check className="h-9 w-9" strokeWidth={2} />
      </motion.div>
      <h1 className="mt-6 text-4xl font-semibold tracking-tight text-zinc-100">
        {target.name} is connected
      </h1>
      <p className="mx-auto mt-3 max-w-md text-zinc-400">
        The cluster checked out. Apps you deploy here land in the{" "}
        <span className="font-mono text-[13px] text-zinc-300">
          {target.cluster?.namespace ?? "projexity"}
        </span>{" "}
        namespace.
      </p>

      {info && (
        <div className="mx-auto mt-8 max-w-md space-y-2 text-left">
          <InfoRow icon={Boxes} label="Kubernetes" value={info.version} />
          <InfoRow
            icon={Cpu}
            label="Nodes"
            value={`${info.node_count} node${info.node_count === 1 ? "" : "s"}`}
          />
          <div className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.06] bg-white/[0.02] px-4 py-2.5 text-sm">
            <span className="flex shrink-0 items-center gap-2.5 text-zinc-500">
              <Globe className="h-4 w-4 text-zinc-600" strokeWidth={1.75} />
              Ingress classes
            </span>
            {info.ingress_classes.length > 0 ? (
              <span className="flex flex-wrap justify-end gap-1.5">
                {info.ingress_classes.map((c) => (
                  <span key={c} className="chip-mono">
                    {c}
                  </span>
                ))}
              </span>
            ) : (
              <span className="font-mono text-[13px] text-amber-400">
                none detected
              </span>
            )}
          </div>
          {info.warnings.map((w) => (
            <div
              key={w}
              className="rounded-lg border border-amber-500/30 bg-amber-500/5 px-4 py-2.5 text-sm text-amber-300"
            >
              {w}
            </div>
          ))}
        </div>
      )}

      <button onClick={onClose} autoFocus className="btn-primary mt-10 px-6 py-3">
        Take me to my targets
      </button>
    </div>
  );
}

function InfoRow({
  icon: Icon,
  label,
  value,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-white/[0.06] bg-white/[0.02] px-4 py-2.5 text-sm">
      <span className="flex items-center gap-2.5 text-zinc-500">
        <Icon className="h-4 w-4 text-zinc-600" strokeWidth={1.75} />
        {label}
      </span>
      <span className="font-mono text-[13px] text-zinc-200">{value}</span>
    </div>
  );
}
