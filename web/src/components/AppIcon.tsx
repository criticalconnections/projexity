const SIZES = {
  sm: { tile: "h-8 w-8 rounded-lg p-1", emoji: "text-base" },
  md: { tile: "h-10 w-10 rounded-xl p-1.5", emoji: "text-xl" },
  lg: { tile: "h-12 w-12 rounded-xl p-2", emoji: "text-2xl" },
} as const;

/** Catalog/app icon tile. `icon` may be an image path (starts with "/",
 * e.g. /app-icons/n8n.png) or an emoji — rendered accordingly in a
 * consistent rounded tile. */
export function AppIcon({
  icon,
  name,
  size = "md",
}: {
  icon: string | undefined;
  name: string;
  size?: keyof typeof SIZES;
}) {
  const s = SIZES[size];
  if (icon && icon.startsWith("/")) {
    return (
      <img
        src={icon}
        alt=""
        aria-hidden
        className={`${s.tile} shrink-0 border border-white/[0.06] bg-white/[0.04] object-contain`}
        title={name}
      />
    );
  }
  return (
    <span
      aria-hidden
      title={name}
      className={`${s.tile} flex shrink-0 items-center justify-center border border-white/[0.06] bg-white/[0.04] ${s.emoji}`}
    >
      {icon ?? "📦"}
    </span>
  );
}
