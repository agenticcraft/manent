import { Moon } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef } from "react";
import { SpeakerChip } from "@/components/SpeakerChip";
import { formatDuration } from "@/lib/format";
import type { Gap, TranscriptSegment, VoiceView } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** Turns up to this many characters are shown on one line with their speaker. */
const SHORT_TURN_CHARS = 60;

function blockText(b: Block): string {
  return b.segments.map((s) => s.text.trim()).join(" ");
}

/** A new block starts when the speaker changes or after a pause this long. */
const BLOCK_GAP_MS = 1500;

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
 * A transcript as speaker blocks (doc-3): a chip, a timestamp, then the text. Captions (no
 * label yet) are light grey; `highlightLast` marks the newest block while recording.
 *
 * With `onSeek` (the meeting screen, VRT-20) every word can be clicked to play from there, and
 * the word and block at `currentMs` are highlighted. The word highlight is set on the DOM
 * directly, so playback doesn't re-render thousands of words several times a second.
 */
export function TranscriptBlocks({
  meetingId,
  segments,
  voices,
  meName,
  onRenameMe,
  highlightLast = false,
  inRoom = false,
  onSeek,
  currentMs,
  focusMs,
  gaps = [],
}: {
  meetingId: number;
  segments: TranscriptSegment[];
  voices: VoiceView[];
  meName: string;
  onRenameMe: (name: string) => void;
  highlightLast?: boolean;
  /** In a room the mic's captions are not the user's alone (VRT-47). */
  inRoom?: boolean;
  onSeek?: (ms: number) => void;
  currentMs?: number;
  /** Scroll this moment into view (a search match). */
  focusMs?: number;
  /** Where the recording lost audio (the computer slept), shown between the blocks. */
  gaps?: Gap[];
}) {
  const blocks = useMemo(() => toBlocks(segments), [segments]);
  const root = useRef<HTMLDivElement>(null);
  const active = useRef<HTMLElement | null>(null);

  // Words by start time. On a recording the mic's and the system's blocks interleave, so the
  // order on screen is not the order in time.
  const words = useRef<{ ms: number; el: HTMLElement }[]>([]);
  useEffect(() => {
    words.current = [...(root.current?.querySelectorAll<HTMLElement>("[data-word-ms]") ?? [])]
      .map((el) => ({ ms: Number(el.dataset.wordMs), el }))
      .sort((x, y) => x.ms - y.ms);
    active.current = null;
  }, [segments]);

  // The word being played (the last one that started) and its block are highlighted.
  useEffect(() => {
    if (currentMs === undefined) return;
    const list = words.current;
    let lo = 0;
    let hi = list.length - 1;
    let found: HTMLElement | null = null;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (list[mid].ms <= currentMs) {
        found = list[mid].el;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    if (found === active.current) return;
    const visible = (el: HTMLElement | null) => {
      if (!el) return false;
      const r = el.getBoundingClientRect();
      return r.bottom > 0 && r.top < window.innerHeight;
    };
    // Follow playback while the previous word was on screen; leave the view alone after the
    // user scrolled away to read.
    const following = active.current === null || visible(active.current);
    active.current?.removeAttribute("data-active");
    active.current?.closest("article")?.removeAttribute("data-current");
    found?.setAttribute("data-active", "true");
    found?.closest("article")?.setAttribute("data-current", "true");
    active.current = found;
    if (found && following && !visible(found)) found.scrollIntoView({ block: "center" });
  }, [currentMs, segments]);

  useEffect(() => {
    if (focusMs === undefined || !root.current) return;
    const target = [...root.current.querySelectorAll<HTMLElement>("[data-block-start]")]
      .filter((el) => Number(el.dataset.blockStart) <= focusMs)
      .pop();
    target?.scrollIntoView({ block: "center" });
  }, [focusMs, blocks]);

  return (
    <div ref={root} className="-mx-3 flex flex-col gap-3 pb-6">
      {blocks.map((b, i) => {
        // Live: the newest block. Meeting: set on the DOM with the playing word (above).
        const current = highlightLast && i === blocks.length - 1;
        const caption = b.label === null;
        // A short turn ("Ja.", "Klopt.") sits on one line with its speaker, so a quick
        // exchange doesn't read as a column of headers (VRT-42).
        // Not the live block that is still growing: it would jump between the two layouts.
        const short = !current && blockText(b).length <= SHORT_TURN_CHARS;
        const prev = i === 0 ? -Infinity : blocks[i - 1].start_ms;
        const before = gaps.filter((g) => g.at_ms > prev && g.at_ms <= b.start_ms);
        return (
          <Fragment key={b.key}>
            {before.map((g) => (
              <GapMarker key={`gap-${g.at_ms}`} gap={g} />
            ))}
            <article
              className={cn(
                "rounded-lg px-3 transition-colors duration-500 data-[current=true]:bg-muted/60",
                short
                  ? "flex flex-wrap items-center gap-x-3 gap-y-1 py-1"
                  : "flex flex-col gap-1 py-2",
              )}
              data-short={short || undefined}
              data-testid="caption-block"
              data-kind={b.kind}
              data-label={b.label ?? ""}
              data-current={highlightLast ? current : undefined}
              data-block-start={b.start_ms}
            >
              <header className="flex items-center gap-2 text-xs">
                <SpeakerChip
                  meetingId={meetingId}
                  voices={voices}
                  label={b.label}
                  kind={b.kind}
                  meName={meName}
                  inRoom={inRoom}
                  onRenameMe={onRenameMe}
                />
                {onSeek ? (
                  <button
                    type="button"
                    className="text-muted-foreground tabular-nums hover:text-foreground"
                    onClick={() => onSeek(b.start_ms)}
                    aria-label={`Play from ${formatDuration(b.start_ms)}`}
                  >
                    {formatDuration(b.start_ms)}
                  </button>
                ) : (
                  <span className="text-muted-foreground tabular-nums">
                    {formatDuration(b.start_ms)}
                  </span>
                )}
              </header>
              <p
                className={cn(
                  "text-[15px] leading-relaxed",
                  caption ? "text-caption" : "text-foreground",
                )}
              >
                {b.segments.map((s) =>
                  onSeek && s.words.length > 0 ? (
                    <span
                      key={`${s.pass}-${s.track_id}-${s.start_ms}`}
                      data-testid="segment"
                      data-start-ms={s.start_ms}
                    >
                      {s.words.map((w, k) => (
                        <span key={k}>
                          <span
                            className="cursor-pointer rounded-sm transition-colors hover:bg-muted data-[active=true]:bg-brand/20"
                            onClick={() => onSeek(w.start_ms)}
                            data-testid="word"
                            data-word-ms={w.start_ms}
                          >
                            {w.text}
                          </span>{" "}
                        </span>
                      ))}
                    </span>
                  ) : (
                    <span
                      key={`${s.pass}-${s.track_id}-${s.start_ms}`}
                      className="animate-in duration-500 fade-in"
                      data-testid={caption ? "caption" : "segment"}
                      data-start-ms={s.start_ms}
                    >
                      {s.text}{" "}
                    </span>
                  ),
                )}
              </p>
            </article>
          </Fragment>
        );
      })}
      {gaps
        .filter((g) => blocks.length === 0 || g.at_ms > blocks[blocks.length - 1].start_ms)
        .map((g) => (
          <GapMarker key={`gap-${g.at_ms}`} gap={g} />
        ))}
    </div>
  );
}

/** A quiet divider where the recording has no audio because the computer slept. */
function GapMarker({ gap }: { gap: Gap }) {
  return (
    <div
      className="flex items-center gap-3 px-3 py-1 text-xs text-muted-foreground"
      role="note"
      data-testid="audio-gap"
      data-at-ms={gap.at_ms}
    >
      <span className="h-px flex-1 bg-border" />
      <span className="flex items-center gap-1.5">
        <Moon className="size-3.5" aria-hidden />
        No audio for {formatGap(gap.missing_ms)} at {formatDuration(gap.at_ms)}: the computer was
        asleep
      </span>
      <span className="h-px flex-1 bg-border" />
    </div>
  );
}

function formatGap(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  const m = Math.round(s / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
}
