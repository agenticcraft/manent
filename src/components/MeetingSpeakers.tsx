import { speakerBg } from "@/lib/format";
import type { TranscriptSegment, VoiceView } from "@/lib/ipc";
import { cn } from "@/lib/utils";

interface Share {
  key: string;
  name: string;
  /** A named person's colour slot, or null. */
  color: number | null;
  /** Colour slot of an unnamed speaker's dot. */
  dot: number | null;
  ms: number;
  firstMs: number;
}

/**
 * Who took part (VRT-42): each speaker of the meeting with their share of the talking, in the
 * meeting's header. Clicking one goes to their first turn. Names as in the transcript.
 */
export function MeetingSpeakers({
  segments,
  voices,
  meName,
  onJump,
}: {
  segments: TranscriptSegment[];
  voices: VoiceView[];
  meName: string;
  onJump: (ms: number) => void;
}) {
  const byKey = new Map<string, Share>();
  for (const s of segments) {
    if (s.label === null || !s.text.trim()) continue; // live captions have no speaker yet
    const voice = voices.find((v) => v.label === s.label);
    const share = byKey.get(s.label) ?? {
      key: s.label,
      name: s.label === "me" ? meName : (voice?.speaker?.name ?? `Speaker ${s.label}`),
      color: voice?.speaker?.color ?? null,
      dot: s.label === "me" || voice?.speaker ? null : Number(s.label),
      ms: 0,
      firstMs: s.start_ms,
    };
    share.ms += Math.max(0, s.end_ms - s.start_ms);
    share.firstMs = Math.min(share.firstMs, s.start_ms);
    byKey.set(s.label, share);
  }
  const shares = [...byKey.values()].sort((a, b) => b.ms - a.ms);
  const total = shares.reduce((n, s) => n + s.ms, 0);
  if (shares.length === 0 || total === 0) return null;

  return (
    <ul
      className="mt-3 flex flex-wrap gap-x-4 gap-y-1.5 text-xs"
      aria-label="Speakers"
      data-testid="meeting-speakers"
    >
      {shares.map((s) => (
        <li key={s.key}>
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-md text-muted-foreground hover:text-foreground"
            onClick={() => onJump(s.firstMs)}
            title={`Go to ${s.name}'s first turn`}
            data-testid="meeting-speaker"
            data-label={s.key}
          >
            <span
              className={cn(
                "size-2 rounded-full",
                s.color !== null
                  ? speakerBg(s.color)
                  : s.dot !== null
                    ? speakerBg(s.dot)
                    : "bg-muted-foreground/40",
              )}
              aria-hidden
            />
            <span className="font-medium text-foreground">{s.name}</span>
            <span className="tabular-nums">{percent(s.ms / total)}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}

/** "54%", or "<1%" for someone who said only a word or two. */
function percent(f: number): string {
  const p = Math.round(f * 100);
  return p < 1 ? "<1%" : `${p}%`;
}
