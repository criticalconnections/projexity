import { useId } from "react";

/** Inline SVG brand mark: rounded hexagon with an upward chevron, filled
 * with an emerald→teal gradient. */
export function BrandMark({ className = "h-6 w-6" }: { className?: string }) {
  const id = useId();
  const grad = `pjx-grad-${id}`;
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <defs>
        <linearGradient id={grad} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#34d399" />
          <stop offset="100%" stopColor="#14b8a6" />
        </linearGradient>
      </defs>
      <path
        d="M12 2.6 20.3 7.3v9.4L12 21.4 3.7 16.7V7.3L12 2.6Z"
        fill={`url(#${grad})`}
        fillOpacity="0.12"
        stroke={`url(#${grad})`}
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      <path
        d="m8.4 13.6 3.6-3.7 3.6 3.7"
        fill="none"
        stroke={`url(#${grad})`}
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/** Brand mark + lowercase wordmark, with an optional alpha chip. */
export function Brand({
  withAlpha = false,
  markClassName = "h-6 w-6",
  wordClassName = "text-lg",
}: {
  withAlpha?: boolean;
  markClassName?: string;
  wordClassName?: string;
}) {
  return (
    <span className="inline-flex items-center gap-2.5">
      <BrandMark className={markClassName} />
      <span
        className={`font-semibold lowercase tracking-tight text-zinc-100 ${wordClassName}`}
      >
        projexity
      </span>
      {withAlpha && (
        <span className="rounded border border-white/10 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400">
          alpha
        </span>
      )}
    </span>
  );
}
