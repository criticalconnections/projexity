import { useEffect, useRef, useState } from "react";

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

/** Scrollable dark mono log panel. Auto-scrolls to the bottom on new lines
 * unless the user has scrolled up to read something. */
export function LogPanel({
  lines,
  connecting,
  error,
  emptyText = "Waiting for output…",
}: {
  lines: string[];
  connecting?: boolean;
  error?: string | null;
  emptyText?: string;
}) {
  const boxRef = useRef<HTMLDivElement>(null);
  const stickRef = useRef(true);

  useEffect(() => {
    const el = boxRef.current;
    if (el && stickRef.current) el.scrollTop = el.scrollHeight;
  }, [lines.length, error]);

  return (
    <div
      ref={boxRef}
      onScroll={(e) => {
        const el = e.currentTarget;
        stickRef.current =
          el.scrollHeight - el.scrollTop - el.clientHeight < 32;
      }}
      className="max-h-[380px] overflow-y-auto rounded-xl border border-zinc-800 bg-zinc-950 p-4 font-mono text-xs leading-relaxed text-zinc-300"
    >
      {lines.length === 0 && !error && (
        <p className="text-zinc-600">{connecting ? "Connecting…" : emptyText}</p>
      )}
      {lines.map((line, i) => (
        <div key={i} className="whitespace-pre-wrap break-all">
          {line || " "}
        </div>
      ))}
      {error && <p className="mt-2 text-red-400">{error}</p>}
    </div>
  );
}
