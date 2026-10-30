import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { AudioPlayer } from "@/components/AudioPlayer";
import { Button } from "@/components/ui/button";
import { formatDateTime, formatDuration } from "@/lib/format";
import { call, type MeetingDetail } from "@/lib/ipc";

/** Meeting detail (doc-3 screen 3). Playback now; transcript and waveform come with VRT-20. */
export function Meeting({ id, onBack }: { id: number; onBack: () => void }) {
  const [meeting, setMeeting] = useState<MeetingDetail | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("get_meeting", { id }).then(setMeeting, (e: unknown) => setError(String(e)));
  }, [id]);

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <div>
        <Button variant="ghost" size="sm" className="-ml-2" onClick={onBack} data-testid="back">
          <ArrowLeft /> Library
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {meeting === null && <p className="text-sm text-muted-foreground">Meeting not found.</p>}
      {meeting && (
        <>
          <header>
            <h1 className="text-xl font-semibold tracking-tight" data-testid="meeting-heading">
              {meeting.title || "Untitled meeting"}
            </h1>
            <p className="mt-1 text-sm text-muted-foreground tabular-nums">
              {formatDateTime(meeting.started_at)} · {formatDuration(meeting.duration_ms)}
            </p>
          </header>
          {meeting.mix_url ? (
            <AudioPlayer src={meeting.mix_url} fallbackDurationMs={meeting.duration_ms} />
          ) : (
            <p className="text-sm text-muted-foreground" data-testid="no-mix">
              No playback file for this meeting.
            </p>
          )}
          <section
            aria-label="Transcript"
            className="rounded-lg border border-dashed px-6 py-16 text-center text-sm text-muted-foreground"
          >
            The transcript appears here once transcription is set up.
          </section>
        </>
      )}
    </div>
  );
}
