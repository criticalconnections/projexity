import { useCallback, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { api, ApiError, type GithubRepo, type Target } from "../api";

type Step = "what" | "where" | "launch";

const STEP_ORDER: Step[] = ["what", "where", "launch"];

/** Typeform-style full-screen new-project wizard: name an image, pick a
 * server, launch. */
export function NewProjectPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const [step, setStep] = useState<Step>("what");
  const [direction, setDirection] = useState(1);
  const [name, setName] = useState("");
  const [source, setSource] = useState<"repo" | "image">("repo");
  const [image, setImage] = useState("");
  const [repo, setRepo] = useState("");
  const [branch, setBranch] = useState("main");
  const [port, setPort] = useState("80");
  const [targetId, setTargetId] = useState<string | null>(null);
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
    queryClient.invalidateQueries({ queryKey: ["projects"] });
    navigate({ to: "/" });
  }, [navigate, queryClient]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const targetsQuery = useQuery({
    queryKey: ["targets"],
    queryFn: api.listTargets,
  });

  const reposQuery = useQuery({
    queryKey: ["github", "repos"],
    queryFn: api.githubRepos,
    enabled: source === "repo",
    staleTime: Infinity,
  });
  const githubConnected = reposQuery.data?.connected ?? false;
  const githubRepos = reposQuery.data?.repos ?? [];
  const readyTargets = (targetsQuery.data ?? []).filter(
    (t) => t.status === "ready",
  );

  // Preselect when there's exactly one ready server.
  useEffect(() => {
    if (!targetId && readyTargets.length === 1) setTargetId(readyTargets[0].id);
  }, [targetId, readyTargets]);

  const launch = useMutation({
    mutationFn: async () => {
      const project = await api.createProject({
        name: name.trim(),
        target_id: targetId!,
        ...(source === "image"
          ? { image: image.trim() }
          : { repo: repo.trim(), branch: branch.trim() || "main" }),
        container_port: parseInt(port, 10) || 80,
      });
      // First deploy kicks off immediately; if it can't start, the project
      // page will say so — the project itself was created fine.
      await api.deployProject(project.id).catch(() => {});
      return project;
    },
    onSuccess: (project) => {
      queryClient.invalidateQueries({ queryKey: ["projects"] });
      navigate({ to: "/projects/$id", params: { id: project.id } });
    },
    onError: (e) =>
      setFormError(e instanceof ApiError ? e.message : "Something went wrong"),
  });

  const stepIndex = STEP_ORDER.indexOf(step);
  const progress = ((stepIndex + 1) / STEP_ORDER.length) * 100;

  const portValid =
    /^\d+$/.test(port.trim()) &&
    parseInt(port, 10) > 0 &&
    parseInt(port, 10) < 65536;
  const sourceValid = source === "image" ? !!image.trim() : !!repo.trim();
  const whatValid = !!name.trim() && sourceValid && portValid;
  const selectedTarget = readyTargets.find((t) => t.id === targetId) ?? null;

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
        {stepIndex > 0 ? (
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
              {step === "what" && (
                <QuestionForm
                  n={1}
                  question="What are we deploying?"
                  hint="Give it a name and tell us which image to run."
                  onSubmit={() => whatValid && go("where")}
                >
                  <BigInput
                    value={name}
                    onChange={setName}
                    placeholder="my-app"
                    autoFocus
                  />
                  <div className="mt-8 flex gap-2">
                    {(
                      [
                        ["repo", "Git repository"],
                        ["image", "Docker image"],
                      ] as const
                    ).map(([key, label]) => (
                      <button
                        key={key}
                        type="button"
                        onClick={() => setSource(key)}
                        className={`rounded-lg border px-4 py-2 text-sm transition ${
                          source === key
                            ? "border-emerald-500 bg-emerald-500/10 text-emerald-300"
                            : "border-zinc-800 text-zinc-400 hover:border-zinc-600"
                        }`}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                  {source === "image" ? (
                    <div className="mt-5">
                      <label className="text-sm text-zinc-500">
                        Docker image
                        <input
                          value={image}
                          onChange={(e) => setImage(e.target.value)}
                          placeholder="nginx:latest"
                          spellCheck={false}
                          autoComplete="off"
                          className="mt-1 block w-full rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 font-mono text-sm text-zinc-200 outline-none transition-colors placeholder:text-zinc-600 focus:border-emerald-500"
                        />
                        <span className="mt-1 block text-xs text-zinc-600">
                          Any public Docker image.
                        </span>
                      </label>
                    </div>
                  ) : (
                    <>
                      {githubConnected && githubRepos.length > 0 && (
                        <RepoPicker
                          repos={githubRepos}
                          onPick={(r) => {
                            setRepo(r.full_name);
                            setBranch(r.default_branch);
                          }}
                        />
                      )}
                      <div className="mt-5 flex gap-4">
                        <label className="flex-1 text-sm text-zinc-500">
                          GitHub repository
                          <input
                            value={repo}
                            onChange={(e) => setRepo(e.target.value)}
                            placeholder="owner/repo"
                            spellCheck={false}
                            autoComplete="off"
                            className="mt-1 block w-full rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 font-mono text-sm text-zinc-200 outline-none transition-colors placeholder:text-zinc-600 focus:border-emerald-500"
                          />
                          <span className="mt-1 block text-xs text-zinc-600">
                            Public repos for now — GitHub App with private
                            repos and push-to-deploy is next. No Dockerfile
                            needed: Node, Python, and static sites are
                            auto-detected.
                          </span>
                          {reposQuery.data && !githubConnected && (
                            <Link
                              to="/settings"
                              className="mt-1 block text-xs text-zinc-500 transition hover:text-emerald-400"
                            >
                              Connect GitHub to pick from your repos and deploy
                              private ones →
                            </Link>
                          )}
                        </label>
                        <label className="w-36 text-sm text-zinc-500">
                          Branch
                          <input
                            value={branch}
                            onChange={(e) => setBranch(e.target.value)}
                            spellCheck={false}
                            autoComplete="off"
                            className="mt-1 block w-full rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 font-mono text-sm text-zinc-200 outline-none focus:border-emerald-500"
                          />
                        </label>
                      </div>
                    </>
                  )}
                  <div className="mt-6">
                    <label className="text-sm text-zinc-500">
                      Container port
                      <input
                        value={port}
                        onChange={(e) => setPort(e.target.value)}
                        inputMode="numeric"
                        className="mt-1 block w-24 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-200 outline-none focus:border-emerald-500"
                      />
                      <span className="mt-1 block text-xs text-zinc-600">
                        The port your app listens on inside the container.
                      </span>
                    </label>
                  </div>
                  <NextButton disabled={!whatValid} />
                </QuestionForm>
              )}

              {step === "where" && (
                <QuestionForm
                  n={2}
                  question="Where should it run?"
                  hint="Pick one of your connected servers."
                  onSubmit={() => targetId && go("launch")}
                >
                  {targetsQuery.isLoading ? (
                    <p className="text-zinc-500">Loading your servers…</p>
                  ) : readyTargets.length === 0 ? (
                    <NoTargets />
                  ) : (
                    <>
                      <div className="space-y-3">
                        {readyTargets.map((t) => (
                          <TargetOption
                            key={t.id}
                            target={t}
                            selected={t.id === targetId}
                            onSelect={() => setTargetId(t.id)}
                          />
                        ))}
                      </div>
                      <NextButton disabled={!targetId} />
                    </>
                  )}
                </QuestionForm>
              )}

              {step === "launch" && (
                <QuestionForm
                  n={3}
                  question="Ready when you are"
                  hint="We'll create the project and start its first deployment right away."
                  onSubmit={() => !launch.isPending && launch.mutate()}
                >
                  <div className="space-y-2">
                    {[
                      ["Name", name.trim()],
                      source === "image"
                        ? ["Image", image.trim()]
                        : ["Repository", `${repo.trim()} (${branch.trim() || "main"})`],
                      ["Container port", port],
                      ["Server", selectedTarget?.name ?? "—"],
                    ].map(([label, value], i) => (
                      <motion.div
                        key={label}
                        initial={{ opacity: 0, x: -10 }}
                        animate={{ opacity: 1, x: 0 }}
                        transition={{ delay: i * 0.07 }}
                        className="flex items-center justify-between rounded-lg border border-zinc-800/70 bg-zinc-900/40 px-4 py-2.5 text-sm"
                      >
                        <span className="text-zinc-500">{label}</span>
                        <span className="font-mono text-zinc-200">{value}</span>
                      </motion.div>
                    ))}
                  </div>
                  {formError && (
                    <p className="mt-4 text-sm text-red-400">{formError}</p>
                  )}
                  <div className="mt-8 flex items-center gap-3">
                    <button
                      type="submit"
                      disabled={launch.isPending}
                      className="rounded-lg bg-emerald-600 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
                    >
                      {launch.isPending ? "Launching…" : "Create & deploy"}
                    </button>
                    <span className="text-xs text-zinc-600">
                      press{" "}
                      <kbd className="rounded border border-zinc-700 px-1">
                        Enter ↵
                      </kbd>
                    </span>
                  </div>
                </QuestionForm>
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

function TargetOption({
  target,
  selected,
  onSelect,
}: {
  target: Target;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={`flex w-full items-center justify-between rounded-xl border px-4 py-3.5 text-left transition ${
        selected
          ? "border-emerald-500 bg-emerald-500/5"
          : "border-zinc-800 bg-zinc-900/40 hover:border-zinc-600"
      }`}
    >
      <span>
        <span className="block font-medium text-zinc-100">{target.name}</span>
        <span className="mt-0.5 block text-sm text-zinc-500">
          {target.ssh_user}@{target.host}
        </span>
      </span>
      <span
        className={`flex h-5 w-5 items-center justify-center rounded-full border text-xs ${
          selected
            ? "border-emerald-500 bg-emerald-500 text-zinc-950"
            : "border-zinc-700 text-transparent"
        }`}
      >
        ✓
      </span>
    </button>
  );
}

/** Compact searchable list of the user's GitHub-App repos. Picking one only
 * pre-fills the manual repo/branch inputs below — those stay the source of
 * truth for submission. */
function RepoPicker({
  repos,
  onPick,
}: {
  repos: GithubRepo[];
  onPick: (repo: GithubRepo) => void;
}) {
  const [filter, setFilter] = useState("");
  const shown = repos
    .filter((r) =>
      r.full_name.toLowerCase().includes(filter.trim().toLowerCase()),
    )
    .slice(0, 6);

  return (
    <div className="mt-5 rounded-lg border border-zinc-800 bg-zinc-900/40 p-3">
      <input
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        placeholder="Search your repositories…"
        spellCheck={false}
        autoComplete="off"
        className="block w-full rounded-md border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-200 outline-none transition-colors placeholder:text-zinc-600 focus:border-emerald-500"
      />
      <div className="mt-2">
        {shown.length === 0 ? (
          <p className="px-2 py-1.5 text-xs text-zinc-600">
            No repositories match "{filter}".
          </p>
        ) : (
          shown.map((r) => (
            <button
              key={r.full_name}
              type="button"
              onClick={() => onPick(r)}
              className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left font-mono text-sm text-zinc-300 transition hover:bg-zinc-800/60 hover:text-zinc-100"
            >
              <span className="truncate">{r.full_name}</span>
              {r.private && (
                <span className="ml-2 shrink-0 rounded bg-zinc-800 px-1.5 py-0.5 font-sans text-[10px] font-medium text-zinc-400">
                  private
                </span>
              )}
            </button>
          ))
        )}
      </div>
    </div>
  );
}

function NoTargets() {
  return (
    <div className="rounded-xl border border-dashed border-zinc-800 p-6">
      <p className="text-zinc-300">No servers are ready yet.</p>
      <p className="mt-1 text-sm text-zinc-500">
        Connect a server first — it takes about two minutes — then come back
        here to deploy onto it.
      </p>
      <Link
        to="/targets/new"
        className="mt-4 inline-block rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500"
      >
        Connect a server
      </Link>
    </div>
  );
}
