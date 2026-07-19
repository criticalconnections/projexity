import { Check, X } from "lucide-react";

/** Shared checklist icon language for bootstrap / install step reports. */
export function StepIcon({
  status,
  size = "md",
}: {
  status: string;
  size?: "sm" | "md";
}) {
  const box = size === "sm" ? "h-4 w-4" : "h-5 w-5";
  const glyph = size === "sm" ? "h-2.5 w-2.5" : "h-3 w-3";
  const spinner = size === "sm" ? "h-3 w-3" : "h-4 w-4";

  if (status === "done" || status === "skipped")
    return (
      <span
        className={`flex ${box} flex-none items-center justify-center rounded-full border border-emerald-500/25 bg-emerald-500/15 text-emerald-400`}
      >
        <Check className={glyph} strokeWidth={2.5} />
      </span>
    );
  if (status === "failed")
    return (
      <span
        className={`flex ${box} flex-none items-center justify-center rounded-full border border-red-500/25 bg-red-500/15 text-red-400`}
      >
        <X className={glyph} strokeWidth={2.5} />
      </span>
    );
  if (status === "running")
    return (
      <span className={`flex ${box} flex-none items-center justify-center`}>
        <span
          className={`${spinner} animate-spin rounded-full border-2 border-white/10 border-t-emerald-400`}
        />
      </span>
    );
  return (
    <span className={`flex ${box} flex-none items-center justify-center`}>
      <span className="h-1.5 w-1.5 rounded-full border border-white/20" />
    </span>
  );
}
