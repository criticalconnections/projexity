import { useEffect, useRef, useState } from "react";
import gsap from "gsap";

export interface SseLog {
  lines: string[];
  /** Fatal stream error (e.g. the server refused the stream). */
  error: string | null;
  /** Data of the terminal `end` event (final status string), if received. */
  ended: string | null;
  /** True while the initial connection is being established. */
  connecting: boolean;
}

/** Subscribe to an SSE log stream (`log` events with JSON `{text}` payloads,
 * terminal `end` event). Pass `null` to disconnect. Reconnects with
 * Last-Event-ID resume are handled natively by EventSource. */
export function useSseLog(
  url: string | null,
  onEnd?: (status: string) => void,
): SseLog {
  const [lines, setLines] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [ended, setEnded] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const onEndRef = useRef(onEnd);
  onEndRef.current = onEnd;

  useEffect(() => {
    setLines([]);
    setError(null);
    setEnded(null);
    if (!url) {
      setConnecting(false);
      return;
    }
    setConnecting(true);
    const es = new EventSource(url);
    let received = false;

    es.onopen = () => {
      setConnecting(false);
      setError(null);
    };
    es.addEventListener("log", (e) => {
      received = true;
      try {
        const data = JSON.parse((e as MessageEvent).data) as { text?: unknown };
        if (typeof data.text === "string") {
          const text = data.text;
          setLines((prev) => [...prev, text]);
        }
      } catch {
        // malformed event; skip
      }
    });
    es.addEventListener("end", (e) => {
      const status = String((e as MessageEvent).data ?? "");
      setEnded(status);
      es.close();
      onEndRef.current?.(status);
    });
    es.onerror = () => {
      // readyState CLOSED means the browser gave up (non-200 response etc.);
      // otherwise it is auto-reconnecting and will resume via Last-Event-ID.
      if (es.readyState === EventSource.CLOSED) {
        es.close();
        setConnecting(false);
        setError(
          received
            ? "Log stream disconnected."
            : "Couldn't open the log stream.",
        );
      }
    };
    return () => es.close();
  }, [url]);

  return { lines, error, ended, connecting };
}

/** Scrollable dark mono log panel with terminal chrome. Auto-scrolls to the
 * bottom on new lines unless the user has scrolled up to read something. */
export function LogPanel({
  lines,
  connecting,
  error,
  emptyText = "Waiting for output…",
  title = "output",
  live = false,
}: {
  lines: string[];
  connecting?: boolean;
  error?: string | null;
  emptyText?: string;
  /** Mono title shown in the terminal header bar. */
  title?: string;
  /** Shows a pulsing "streaming" indicator in the header bar. */
  live?: boolean;
}) {
  const boxRef = useRef<HTMLDivElement>(null);
  const stickRef = useRef(true);
  const animatedCountRef = useRef(0);

  useEffect(() => {
    const el = boxRef.current;
    if (el && stickRef.current) el.scrollTop = el.scrollHeight;
  }, [lines.length, error]);

  // Animate freshly appended lines in (opacity 0→1, x -4→0). Purely visual;
  // skipped for large bursts and when the user prefers reduced motion.
  useEffect(() => {
    const el = boxRef.current;
    const prev = animatedCountRef.current;
    animatedCountRef.current = lines.length;
    if (!el || lines.length <= prev) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const fresh = Array.from(el.querySelectorAll("[data-log-line]")).slice(
      prev,
    );
    if (fresh.length === 0 || fresh.length > 40) return;
    gsap.fromTo(
      fresh,
      { opacity: 0, x: -4 },
      {
        opacity: 1,
        x: 0,
        duration: 0.12,
        ease: "power1.out",
        stagger: 0.015,
        overwrite: true,
        clearProps: "opacity,transform",
      },
    );
  }, [lines.length]);

  return (
    <div className="overflow-hidden rounded-xl border border-white/[0.06] bg-[#0b0b0d] shadow-lg shadow-black/40">
      {/* terminal chrome */}
      <div className="flex items-center gap-3 border-b border-white/[0.06] bg-white/[0.02] px-4 py-2">
        <span className="flex gap-1.5" aria-hidden>
          <span className="h-2.5 w-2.5 rounded-full bg-white/10" />
          <span className="h-2.5 w-2.5 rounded-full bg-white/10" />
          <span className="h-2.5 w-2.5 rounded-full bg-white/10" />
        </span>
        <span className="min-w-0 truncate font-mono text-[11px] text-zinc-500">
          {title}
        </span>
        {live && (
          <span className="ml-auto flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-wider text-emerald-400">
            <span className="relative flex h-1.5 w-1.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-60" />
              <span className="relative inline-flex h-1.5 w-1.5 rounded-full bg-emerald-400" />
            </span>
            streaming
          </span>
        )}
      </div>
      <div
        ref={boxRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          stickRef.current =
            el.scrollHeight - el.scrollTop - el.clientHeight < 32;
        }}
        className="max-h-[380px] overflow-y-auto p-4 font-mono text-[12.5px] leading-relaxed text-zinc-300"
      >
        {lines.length === 0 && !error && (
          <p className="text-zinc-600">
            {connecting ? "Connecting…" : emptyText}
          </p>
        )}
        {lines.map((line, i) => (
          <div
            key={i}
            data-log-line
            className="-mx-2 whitespace-pre-wrap break-all rounded px-2 hover:bg-white/[0.02]"
          >
            {line || " "}
          </div>
        ))}
        {error && <p className="mt-2 text-red-400">{error}</p>}
      </div>
    </div>
  );
}
