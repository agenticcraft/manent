import { ArrowDown } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { formatDuration } from "@/lib/format";
import { call, on, type Caption, type RecordingStarted } from "@/lib/ipc";

/** A new block starts when the source changes or after a pause this long. */
const BLOCK_GAP_MS = 1500;
/** Within this distance from the bottom the view keeps following new text. */
const FOLLOW_PX = 48;

interface Block {
  key: string;
  kind: Caption["kind"];
  start_ms: number;
  captions: Caption[];
}

function insert(list: Caption[], c: Caption): Caption[] {
  if (list.some((x) => x.track_id === c.track_id && x.start_ms === c.start_ms)) return list;
  const next = [...list, c];
  next.sort((a, b) => a.start_ms - b.start_ms || a.track_id - b.track_id);
  return next;
}

function toBlocks(captions: Caption[]): Block[] {
  const blocks: Block[] = [];
  for (const c of captions) {
    const last = blocks[blocks.length - 1] as Block | undefined;
    const lastEnd = last ? Math.max(...last.captions.map((x) => x.end_ms)) : 0;
    if (last && last.captions[0].track_id === c.track_id && c.start_ms - lastEnd < BLOCK_GAP_MS) {
      last.captions.push(c);
    } else {
      blocks.push({
        key: `${c.track_id}-${c.start_ms}`,
        kind: c.kind,
        start_ms: c.start_ms,
        captions: [c],
      });
    }
  }
  return blocks;
}

/**
 * Live captions (VRT-16): light grey text per source, in time order. The mic is "Me";
 * speakers on the system audio are added by the rolling pass (VRT-17).
 */
export function LiveTranscript({
  session,
  paused,
}: {
  session: RecordingStarted;
  paused: boolean;
}) {
  const [captions, setCaptions] = useState<Caption[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [following, setFollowing] = useState(true);
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = session.meeting_id;
    call("live_captions", { meetingId: id }).then(
      (list) => setCaptions((cur) => list.reduce(insert, cur)),
      () => {},
    );
    const offCaption = on("transcript:caption", (c) => {
      if (c.meeting_id !== id) return;
      setCaptions((cur) => insert(cur, c));
      setError(null);
    });
    const offError = on("transcript:error", (e) => {
      if (e.meeting_id === id) setError(e.message);
    });
    return () => {
      offCaption.then((f) => f());
      offError.then((f) => f());
    };
  }, [session.meeting_id]);

  const blocks = useMemo(() => toBlocks(captions), [captions]);

  // Keep the newest text in view unless the user scrolled up to read.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && following) el.scrollTop = el.scrollHeight;
  }, [blocks, following]);

  const onScroll = () => {
    const el = scroller.current;
    if (el) setFollowing(el.scrollHeight - el.scrollTop - el.clientHeight < FOLLOW_PX);
  };

  const jumpToLive = () => {
    const el = scroller.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
    setFollowing(true);
  };

  if (!session.captions) {
    return (
      <Placeholder>
        Recording. Live captions are off: download the transcription engine in Settings.
      </Placeholder>
    );
  }
  if (blocks.length === 0) {
    return (
      <Placeholder>
        {paused
          ? "Paused. Nothing is recorded until you resume."
          : "Listening… the first words appear after a few seconds."}
        {error && <span className="mt-2 block text-destructive">{error}</span>}
      </Placeholder>
    );
  }

  return (
    <section aria-label="Live transcript" className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={scroller}
        onScroll={onScroll}
        className="min-h-0 flex-1 overflow-y-auto pr-2"
        data-testid="live-transcript"
      >
        <div className="flex max-w-3xl flex-col gap-5 pb-6">
          {blocks.map((b) => (
            <article
              key={b.key}
              className="flex flex-col gap-1"
              data-testid="caption-block"
              data-kind={b.kind}
            >
              <header className="flex items-center gap-2 text-xs">
                <span className="rounded-full bg-muted px-2 py-0.5 font-medium text-foreground">
                  {b.kind === "mic" ? "Me" : "System audio"}
                </span>
                <span className="text-muted-foreground tabular-nums">
                  {formatDuration(b.start_ms)}
                </span>
              </header>
              <p className="text-[15px] leading-relaxed text-caption">
                {b.captions.map((c) => (
                  <span
                    key={c.start_ms}
                    className="animate-in duration-500 fade-in"
                    data-testid="caption"
                    data-start-ms={c.start_ms}
                  >
                    {c.text}{" "}
                  </span>
                ))}
              </p>
            </article>
          ))}
        </div>
      </div>
      {error && (
        <p className="pt-2 text-xs text-destructive" role="status" data-testid="caption-error">
          Captions skipped a moment: {error}
        </p>
      )}
      {!following && (
        <button
          type="button"
          onClick={jumpToLive}
          className="absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full border bg-background px-3 py-1.5 text-xs font-medium shadow-sm"
          data-testid="jump-to-live"
        >
          <ArrowDown className="size-3.5" /> Jump to live
        </button>
      )}
    </section>
  );
}

function Placeholder({ children }: { children: React.ReactNode }) {
  return (
    <section
      aria-label="Live transcript"
      className="flex flex-1 items-center justify-center rounded-lg border border-dashed"
      data-testid="live-placeholder"
    >
      <p className="max-w-sm text-center text-sm text-muted-foreground">{children}</p>
    </section>
  );
}
