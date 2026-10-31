import { ArrowDown } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { TranscriptBlocks } from "@/components/TranscriptBlocks";
import { sorted, useSpeakers } from "@/lib/transcript";
import { call, on, type RecordingStarted, type TranscriptSegment } from "@/lib/ipc";

/** Within this distance from the bottom the view keeps following new text. */
const FOLLOW_PX = 48;

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
  const { voices, meName, renameMe } = useSpeakers(session.meeting_id, setError);
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = session.meeting_id;
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

  // Keep the newest text in view unless the user scrolled up to read.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && following) el.scrollTop = el.scrollHeight;
  }, [segments, following]);

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
  if (segments.length === 0) {
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
        <TranscriptBlocks
          meetingId={session.meeting_id}
          segments={segments}
          voices={voices}
          meName={meName}
          onRenameMe={renameMe}
          highlightLast
        />
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
