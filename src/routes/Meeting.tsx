import { ArrowLeft, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { AudioPlayer } from "@/components/AudioPlayer";
import { TranscriptBlocks } from "@/components/TranscriptBlocks";
import { Button } from "@/components/ui/button";
import { formatDateTime, formatDuration } from "@/lib/format";
import { call, on, type JobView, type MeetingDetail, type TranscriptSegment } from "@/lib/ipc";
import { sorted, useSpeakers } from "@/lib/transcript";

/**
 * Meeting detail (doc-3 screen 3): playback and the transcript with named speakers, and
 * "Re-transcribe" (VRT-19). Click-to-seek, search and the waveform come with VRT-20.
 */
export function Meeting({ id, onBack }: { id: number; onBack: () => void }) {
  const [meeting, setMeeting] = useState<MeetingDetail | null | undefined>(undefined);
  const [segments, setSegments] = useState<TranscriptSegment[]>([]);
  const [job, setJob] = useState<JobView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { voices, meName, renameMe } = useSpeakers(id, setError);

  const load = useCallback(() => {
    call("get_meeting", { id }).then(setMeeting, (e: unknown) => setError(String(e)));
    call("live_transcript", { meetingId: id }).then(
      (list) => setSegments(sorted(list)),
      (e: unknown) => setError(String(e)),
    );
  }, [id]);
  useEffect(load, [load]);

  // A job for this meeting (import, final pass or re-transcribe): show it, reload when done.
  useEffect(() => {
    const pick = (jobs: JobView[]) => {
      const mine = jobs.filter((j) => j.meeting_id === id && j.stage !== "done");
      return mine.length > 0 ? mine[mine.length - 1] : null;
    };
    call("list_jobs").then(
      (jobs) => setJob(pick(jobs)),
      () => {},
    );
    const offJobs = on("jobs:changed", (jobs) => {
      setJob(pick(jobs));
      if (jobs.some((j) => j.meeting_id === id && j.stage === "done")) load();
    });
    const offFinal = on("transcript:final", (e) => {
      if (e.meeting_id === id) setSegments(sorted(e.segments));
    });
    return () => {
      offJobs.then((f) => f());
      offFinal.then((f) => f());
    };
  }, [id, load]);

  const busy = job !== null && job.stage !== "failed";
  const retranscribe = async () => {
    setError(null);
    try {
      await call("retranscribe", { meetingId: id });
    } catch (e) {
      setError(String(e));
    }
  };

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
          <header className="flex items-start justify-between gap-4">
            <div>
              <h1 className="text-xl font-semibold tracking-tight" data-testid="meeting-heading">
                {meeting.title || "Untitled meeting"}
              </h1>
              <p className="mt-1 text-sm text-muted-foreground tabular-nums">
                {formatDateTime(meeting.started_at)} · {formatDuration(meeting.duration_ms)}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={retranscribe}
              disabled={busy || meeting.status === "recording"}
              data-testid="retranscribe"
            >
              <RefreshCw /> Re-transcribe
            </Button>
          </header>
          {meeting.mix_url ? (
            <AudioPlayer src={meeting.mix_url} fallbackDurationMs={meeting.duration_ms} />
          ) : (
            <p className="text-sm text-muted-foreground" data-testid="no-mix">
              No playback file for this meeting.
            </p>
          )}
          {job && (
            <div className="flex flex-col gap-1.5" data-testid="meeting-job" data-stage={job.stage}>
              <p
                className={
                  job.stage === "failed"
                    ? "text-sm text-destructive"
                    : "text-sm text-muted-foreground"
                }
              >
                {job.stage === "failed"
                  ? `Transcription failed: ${job.error ?? "unknown error"}`
                  : job.stage === "waiting"
                    ? "Waiting to be transcribed…"
                    : `Transcribing… ${Math.round(job.progress * 100)}%`}
              </p>
              {job.stage !== "failed" && (
                <div className="h-1 overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full rounded-full bg-brand transition-[width] duration-500"
                    style={{ width: `${job.progress * 100}%` }}
                  />
                </div>
              )}
            </div>
          )}
          {segments.length > 0 ? (
            <section aria-label="Transcript" data-testid="meeting-transcript">
              <TranscriptBlocks
                meetingId={id}
                segments={segments}
                voices={voices}
                meName={meName}
                onRenameMe={renameMe}
              />
            </section>
          ) : (
            !busy && (
              <section
                aria-label="Transcript"
                className="rounded-lg border border-dashed px-6 py-16 text-center text-sm text-muted-foreground"
              >
                No transcript yet. Use Re-transcribe once the transcription engine is downloaded.
              </section>
            )
          )}
        </>
      )}
    </div>
  );
}
