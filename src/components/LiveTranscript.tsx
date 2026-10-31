import { ArrowDown } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { formatDuration, speakerBg } from "@/lib/format";
import { call, on, type RecordingStarted, type TranscriptSegment } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** A new block starts when the speaker changes or after a pause this long. */
const BLOCK_GAP_MS = 1500;
/** Within this distance from the bottom the view keeps following new text. */
const FOLLOW_PX = 48;

interface Block {
  key: string;
  /** Who: a pass label ("me", "1", ...) or, for captions, the source kind. */
  who: string;
  label: string | null;
  kind: TranscriptSegment["kind"];
  start_ms: number;
  end_ms: number;
  segments: TranscriptSegment[];
}

const whoOf = (s: TranscriptSegment) => s.label ?? `caption-${s.kind}`;

function sorted(list: TranscriptSegment[]): TranscriptSegment[] {
  return [...list].sort((a, b) => a.start_ms - b.start_ms || a.track_id - b.track_id);
}

function toBlocks(segments: TranscriptSegment[]): Block[] {
  const blocks: Block[] = [];
  for (const s of segments) {
    const last = blocks[blocks.length - 1] as Block | undefined;
    if (last && last.who === whoOf(s) && s.start_ms - last.end_ms < BLOCK_GAP_MS) {
      last.segments.push(s);
      last.end_ms = Math.max(last.end_ms, s.end_ms);
    } else {
      blocks.push({
        key: `${whoOf(s)}-${s.track_id}-${s.start_ms}`,
        who: whoOf(s),
        label: s.label,
        kind: s.kind,
        start_ms: s.start_ms,
        end_ms: s.end_ms,
        segments: [s],
      });
    }
  }
  return blocks;
}

/**
 * The live transcript (VRT-16/VRT-17): captions appear first in light grey per source; every
 * rolling pass replaces them with speaker blocks ("Me" for the mic, "Speaker N" for voices on
 * the system audio). The newest block is highlighted.
 */
export function LiveTranscript({
  session,
  paused,
}: {
  session: RecordingStarted;
  paused: boolean;
}) {
  const [segments, setSegments] = useState<TranscriptSegment[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [following, setFollowing] = useState(true);
  const [meName, setMeName] = useState("Me");
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = session.meeting_id;
    call("get_me_name").then(setMeName, () => {});
    call("live_transcript", { meetingId: id }).then(
      (list) => setSegments((cur) => (cur.length ? cur : sorted(list))),
      () => {},
    );
    const offCaption = on("transcript:caption", (c) => {
      if (c.meeting_id !== id) return;
      setSegments((cur) =>
        cur.some(
          (x) => x.pass === "caption" && x.track_id === c.track_id && x.start_ms === c.start_ms,
        )
          ? cur
          : sorted([...cur, { ...c, label: null, pass: "caption" }]),
      );
      setError(null);
    });
    const offPass = on("transcript:pass", (p) => {
      if (p.meeting_id !== id) return;
      setSegments((cur) =>
        sorted([
          ...cur.filter(
            (s) => s.pass === "final" || s.start_ms < p.from_ms || s.start_ms >= p.to_ms,
          ),
          ...p.segments,
        ]),
      );
    });
    const offError = on("transcript:error", (e) => {
      if (e.meeting_id === id) setError(e.message);
    });
    return () => {
      for (const off of [offCaption, offPass, offError]) off.then((f) => f());
    };
  }, [session.meeting_id]);

  const blocks = useMemo(() => toBlocks(segments), [segments]);

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

  const renameMe = async (name: string) => {
    const next = name.trim() || "Me";
    setMeName(next);
    try {
      await call("set_me_name", { name: next });
    } catch (e) {
      setError(String(e));
    }
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
        <div className="flex max-w-3xl flex-col gap-3 pb-6">
          {blocks.map((b, i) => {
            const current = i === blocks.length - 1;
            const caption = b.label === null;
            return (
              <article
                key={b.key}
                className={cn(
                  "flex flex-col gap-1 rounded-lg px-3 py-2 transition-colors duration-500",
                  current && "bg-muted/60",
                )}
                data-testid="caption-block"
                data-kind={b.kind}
                data-label={b.label ?? ""}
                data-current={current}
              >
                <header className="flex items-center gap-2 text-xs">
                  <SpeakerChip
                    label={b.label}
                    kind={b.kind}
                    meName={meName}
                    onRenameMe={renameMe}
                  />
                  <span className="text-muted-foreground tabular-nums">
                    {formatDuration(b.start_ms)}
                  </span>
                </header>
                <p
                  className={cn(
                    "text-[15px] leading-relaxed",
                    caption ? "text-caption" : "text-foreground",
                  )}
                >
                  {b.segments.map((s) => (
                    <span
                      key={`${s.pass}-${s.track_id}-${s.start_ms}`}
                      className="animate-in duration-500 fade-in"
                      data-testid={caption ? "caption" : "segment"}
                      data-start-ms={s.start_ms}
                    >
                      {s.text}{" "}
                    </span>
                  ))}
                </p>
              </article>
            );
          })}
        </div>
      </div>
      {error && (
        <p className="pt-2 text-xs text-destructive" role="status" data-testid="caption-error">
          Transcription skipped a moment: {error}
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

/** Who spoke: "Me" (renamable), "Speaker N" in its colour, or the source of a caption. */
function SpeakerChip({
  label,
  kind,
  meName,
  onRenameMe,
}: {
  label: string | null;
  kind: TranscriptSegment["kind"];
  meName: string;
  onRenameMe: (name: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const chip = "rounded-full px-2 py-0.5 font-medium text-foreground";
  if (label === "me" || (label === null && kind === "mic")) {
    if (editing) {
      return (
        <input
          autoFocus
          defaultValue={meName}
          aria-label="Your name"
          className={cn(chip, "w-32 bg-muted ring-2 ring-ring/50 outline-none")}
          onBlur={(e) => {
            setEditing(false);
            onRenameMe(e.currentTarget.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            if (e.key === "Escape") setEditing(false);
          }}
          data-testid="me-name-input"
        />
      );
    }
    return (
      <button
        type="button"
        className={cn(chip, "bg-muted hover:bg-muted/70")}
        title="Rename"
        onClick={() => setEditing(true)}
        data-testid="speaker-chip"
        data-speaker="me"
      >
        {meName}
      </button>
    );
  }
  if (label === null) {
    return <span className={cn(chip, "bg-muted")}>System audio</span>;
  }
  return (
    <span
      className={cn(chip, speakerBg(Number(label)))}
      data-testid="speaker-chip"
      data-speaker={label}
    >
      Speaker {label}
    </span>
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
