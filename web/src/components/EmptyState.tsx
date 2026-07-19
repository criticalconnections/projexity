interface EmptyStateProps {
  title: string;
  description: string;
  actionLabel?: string;
}

export function EmptyState({ title, description, actionLabel }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-zinc-800 py-24 text-center">
      <h2 className="text-lg font-medium text-zinc-200">{title}</h2>
      <p className="mt-2 max-w-md text-sm text-zinc-500">{description}</p>
      {actionLabel && (
        <button
          disabled
          title="Coming in the next milestone"
          className="mt-6 cursor-not-allowed rounded-md bg-emerald-600/50 px-4 py-2 text-sm font-medium text-white/70"
        >
          {actionLabel}
        </button>
      )}
    </div>
  );
}
