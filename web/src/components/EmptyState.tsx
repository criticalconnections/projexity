import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface EmptyStateProps {
  icon?: LucideIcon;
  title: string;
  description: string;
  action?: ReactNode;
}

/** Consistent empty state: faint dot-grid background, icon tile, heading,
 * copy, and an optional call-to-action. */
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <div className="dot-grid relative flex flex-col items-center justify-center overflow-hidden rounded-xl border border-dashed border-white/[0.08] py-24 text-center">
      {Icon && (
        <div className="mb-5 flex h-12 w-12 items-center justify-center rounded-xl border border-white/[0.06] bg-white/[0.03] text-zinc-400 shadow-lg shadow-black/40">
          <Icon className="h-5 w-5" strokeWidth={1.75} />
        </div>
      )}
      <h2 className="text-lg font-medium tracking-tight text-zinc-200">
        {title}
      </h2>
      <p className="mt-2 max-w-md text-sm text-zinc-500">{description}</p>
      {action && <div className="mt-6">{action}</div>}
    </div>
  );
}
