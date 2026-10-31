import { useMemo } from "react";
import { SpeakerChip } from "@/components/SpeakerChip";
import { formatDuration } from "@/lib/format";
import type { TranscriptSegment, VoiceView } from "@/lib/ipc";
import { cn } from "@/lib/utils";

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
 */
export function TranscriptBlocks({
  meetingId,
  segments,
  voices,
  meName,
  onRenameMe,
  highlightLast = false,
}: {
  meetingId: number;
  segments: TranscriptSegment[];
  voices: VoiceView[];
  meName: string;
  onRenameMe: (name: string) => void;
  highlightLast?: boolean;
}) {
  const blocks = useMemo(() => toBlocks(segments), [segments]);
  return (
    <div className="flex max-w-3xl flex-col gap-3 pb-6">
      {blocks.map((b, i) => {
        const current = highlightLast && i === blocks.length - 1;
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
                meetingId={meetingId}
                voices={voices}
                label={b.label}
                kind={b.kind}
                meName={meName}
                onRenameMe={onRenameMe}
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
  );
}
