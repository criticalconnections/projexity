import { useCallback, useEffect } from "react";
import { useNavigate } from "@tanstack/react-router";
import { motion, useReducedMotion } from "motion/react";
import { ArrowRight, Boxes, Server, X } from "lucide-react";

/** Full-screen chooser between the two connect flows: a Linux server over
 * SSH (Docker) or an existing Kubernetes cluster via kubeconfig. */
export function ChooseTargetPage() {
  const navigate = useNavigate();
  const reducedMotion = useReducedMotion();

  const close = useCallback(() => {
    navigate({ to: "/targets" });
  }, [navigate]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-canvas">
      {/* header */}
      <div className="flex items-center justify-end px-6 py-4">
        <button onClick={close} title="Close" className="btn-ghost px-2 py-1.5">
          <X className="h-4 w-4" strokeWidth={1.75} />
        </button>
      </div>

      {/* body */}
      <div className="flex flex-1 items-center justify-center overflow-y-auto px-6 pb-16">
        <div className="w-full max-w-2xl">
          <motion.div
            initial={reducedMotion ? false : { opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.28, ease: "easeOut" }}
          >
            <h1 className="text-4xl font-semibold tracking-tight text-zinc-100">
              What are we connecting?
            </h1>
            <p className="mt-3 text-zinc-500">
              Both end up in the same place — targets your apps deploy onto.
            </p>
            <div className="mt-8 grid gap-4 sm:grid-cols-2">
              <ChoiceCard
                icon={Server}
                title="Linux server"
                description="Any VPS or machine with SSH. We install Docker + a reverse proxy."
                delay={0.1}
                reducedMotion={reducedMotion}
                autoFocus
                onSelect={() => navigate({ to: "/targets/new" })}
              />
              <ChoiceCard
                icon={Boxes}
                title="Kubernetes cluster"
                description="Deploy onto your existing cluster via kubeconfig."
                delay={0.18}
                reducedMotion={reducedMotion}
                onSelect={() => navigate({ to: "/targets/new-cluster" })}
              />
            </div>
          </motion.div>
        </div>
      </div>
    </div>
  );
}

function ChoiceCard({
  icon: Icon,
  title,
  description,
  delay,
  reducedMotion,
  autoFocus,
  onSelect,
}: {
  icon: typeof Server;
  title: string;
  description: string;
  delay: number;
  reducedMotion: boolean | null;
  autoFocus?: boolean;
  onSelect: () => void;
}) {
  return (
    <motion.button
      type="button"
      onClick={onSelect}
      autoFocus={autoFocus}
      initial={reducedMotion ? false : { opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay, duration: 0.24, ease: "easeOut" }}
      className="card card-hover group flex flex-col items-start p-6 text-left transition-colors hover:border-emerald-500/40"
    >
      <span className="flex h-12 w-12 items-center justify-center rounded-xl border border-emerald-500/20 bg-emerald-500/10 text-emerald-400">
        <Icon className="h-6 w-6" strokeWidth={1.5} />
      </span>
      <span className="mt-4 flex items-center gap-1.5 text-lg font-medium tracking-tight text-zinc-100">
        {title}
        <ArrowRight
          className="h-4 w-4 text-zinc-600 transition-all duration-150 group-hover:translate-x-0.5 group-hover:text-emerald-400"
          strokeWidth={1.75}
        />
      </span>
      <span className="mt-1.5 text-sm leading-relaxed text-zinc-500">
        {description}
      </span>
    </motion.button>
  );
}
