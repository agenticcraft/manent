import { ArrowLeft, Pencil, RefreshCw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { AudioPlayer, type PlayerHandle } from "@/components/AudioPlayer";
import { ExportMenu } from "@/components/ExportMenu";
import { MeetingSpeakers } from "@/components/MeetingSpeakers";
import { TranscriptBlocks } from "@/components/TranscriptBlocks";
import { Button } from "@/components/ui/button";
import { formatDateTime, formatDuration, meetingTitle } from "@/lib/format";
import {
  call,
  on,
  type Gap,
  type JobView,
  type Mark,
  type MeetingDetail,
  type TranscriptSegment,
} from "@/lib/ipc";
import { sorted, useSpeakers } from "@/lib/transcript";

/**
 * Meeting detail (doc-3 screen 3): playback and the transcript with named speakers, and
 * "Re-transcribe" (VRT-19). Click-to-seek, search and the waveform come with VRT-20.
 */
export function Meeting({
  id,
  focusMs,
  onBack,
}: {
  id: number;
  /** Open at this moment (a search match): scroll there and cue the player. */
  focusMs?: number;
  onBack: () => void;
}) {
  const [meeting, setMeeting] = useState<MeetingDetail | null | undefined>(undefined);
  const [segments, setSegments] = useState<TranscriptSegment[]>([]);
  const [job, setJob] = useState<JobView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { voices, meName, renameMe } = useSpeakers(id, setError);
  const player = useRef<PlayerHandle>(null);
  const [currentMs, setCurrentMs] = useState<number | undefined>(focusMs);
  const [waveform, setWaveform] = useState<number[]>([]);
  const [gaps, setGaps] = useState<Gap[]>([]);

  useEffect(() => {
    call("meeting_gaps", { meetingId: id }).then(setGaps, () => {});
  }, [id]);
  // Moments marked while recording (VRT-52).
  const [marks, setMarks] = useState<Mark[]>([]);
  useEffect(() => {
    const load = () => call("list_marks", { meetingId: id }).then(setMarks, () => {});
    load();
    const off = on("meeting:marks", (e) => {
      if (e.meeting_id === id) load();
    });
    return () => {
      off.then((f) => f());
    };
  }, [id]);
  const labelMark = (markId: number, label: string) =>
    call("set_mark_label", { id: markId, label }).catch((e: unknown) => setError(String(e)));
  const deleteMark = (markId: number) =>
    call("delete_mark", { id: markId }).catch((e: unknown) => setError(String(e)));
  // The waveform comes with the playback file (VRT-45): ask once that exists.
  const hasMix = Boolean(meeting?.mix_url);
  useEffect(() => {
    if (hasMix) call("meeting_waveform", { id }).then(setWaveform, () => {});
  }, [id, hasMix]);

  // A search match: cue the player there once it exists (it mounts with the meeting).
  useEffect(() => {
    if (focusMs !== undefined && meeting?.mix_url) player.current?.seek(focusMs);
  }, [focusMs, meeting?.mix_url]);

  const seek = (ms: number) => {
    player.current?.seek(ms, true);
    setCurrentMs(ms);
  };

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
    // Just stopped: the player appears when the playback file is ready.
    const offPlayback = on("meeting:playback", (meetingId) => {
      if (meetingId === id) {
        call("get_meeting", { id }).then(setMeeting, (e: unknown) => setError(String(e)));
      }
    });
    return () => {
      offJobs.then((f) => f());
      offFinal.then((f) => f());
      offPlayback.then((f) => f());
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
    <div className="flex flex-col gap-6">
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
            <div className="min-w-0 flex-1">
              <MeetingTitle
                meeting={meeting}
                onRenamed={(title) => setMeeting({ ...meeting, title })}
                onError={setError}
              />
              <p className="mt-1 text-sm text-muted-foreground tabular-nums">
                {formatDateTime(meeting.started_at)} · {formatDuration(meeting.duration_ms)}
              </p>
              <MeetingSpeakers
                segments={segments}
                voices={voices}
                meName={meName}
                onJump={(ms) => {
                  // To their first turn: in view and cued in the player (not playing).
                  document
                    .querySelector(`[data-block-start="${ms}"]`)
                    ?.scrollIntoView({ block: "center", behavior: "smooth" });
                  player.current?.seek(ms);
                  setCurrentMs(ms);
                }}
              />
              {meeting.interrupted && (
                <p
                  className="mt-2 flex items-center gap-1.5 text-xs text-muted-foreground"
                  role="note"
                  data-testid="interrupted-note"
                >
                  <TriangleAlert className="size-3.5" aria-hidden />
                  Manent quit unexpectedly during this recording. Everything up to{" "}
                  {formatDuration(meeting.duration_ms)} was recovered.
                </p>
              )}
            </div>
            <div className="flex gap-2">
              <ExportMenu
                meetingId={id}
                title={meetingTitle(meeting).text}
                dateLabel={formatDateTime(meeting.started_at)}
                disabled={segments.length === 0}
                onError={setError}
              />
              <Button
                variant="outline"
                size="sm"
                onClick={retranscribe}
                disabled={busy || meeting.status === "recording"}
                data-testid="retranscribe"
              >
                <RefreshCw /> Re-transcribe
              </Button>
            </div>
          </header>
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
                    : job.stage === "mixing"
                      ? `Preparing audio… ${Math.round(job.progress * 100)}%`
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
          {segments.length === 0 && (gaps.length > 0 || marks.length > 0) && (
            // No words, but the recording lost audio: still say where.
            <TranscriptBlocks
              meetingId={id}
              segments={[]}
              voices={voices}
              meName={meName}
              onRenameMe={renameMe}
              onSeek={meeting.mix_url ? seek : undefined}
              gaps={gaps}
              marks={marks}
              onLabelMark={labelMark}
              onDeleteMark={deleteMark}
            />
          )}
          {segments.length > 0 ? (
            <section aria-label="Transcript" data-testid="meeting-transcript">
              <TranscriptBlocks
                meetingId={id}
                segments={segments}
                voices={voices}
                meName={meName}
                onRenameMe={renameMe}
                inRoom={meeting.in_room}
                onSeek={meeting.mix_url ? seek : undefined}
                currentMs={currentMs}
                focusMs={focusMs}
                gaps={gaps}
                marks={marks}
                onLabelMark={labelMark}
                onDeleteMark={deleteMark}
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
          {meeting.mix_url ? (
            // The player stays at the bottom of the screen while the transcript scrolls (doc-3).
            // -bottom-8 + pb-12: it also covers the screen's 2 rem bottom padding.
            <div className="sticky -bottom-8 z-10 -mx-1 bg-background px-1 pt-2 pb-12">
              <AudioPlayer
                ref={player}
                src={meeting.mix_url}
                fallbackDurationMs={meeting.duration_ms}
                waveform={waveform}
                marks={marks.map((m) => m.at_ms)}
                onTime={setCurrentMs}
              />
            </div>
          ) : job?.stage === "mixing" ? null : ( // the job line above says "Preparing audio…"
            <p className="text-sm text-muted-foreground" data-testid="no-mix">
              No playback file for this meeting.
            </p>
          )}
        </>
      )}
    </div>
  );
}

/**
 * The meeting's title, renamed in place (VRT-46): click it, type, Enter or leaving the field
 * saves, Esc cancels; empty goes back to the automatic title.
 */
function MeetingTitle({
  meeting,
  onRenamed,
  onError,
}: {
  meeting: MeetingDetail;
  onRenamed: (title: string) => void;
  onError: (e: string) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const cancelled = useRef(false);
  const auto = meetingTitle({ title: "", started_at: meeting.started_at }).text;

  const save = async () => {
    if (draft === null) return;
    const title = draft.trim();
    setDraft(null);
    if (cancelled.current || title === meeting.title) return;
    try {
      await call("rename_meeting", { id: meeting.id, title });
      onRenamed(title);
    } catch (e) {
      onError(String(e));
    }
  };

  if (draft !== null) {
    return (
      <input
        autoFocus
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onFocus={(e) => e.currentTarget.select()}
        onBlur={save}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") {
            cancelled.current = true;
            e.currentTarget.blur();
          }
        }}
        placeholder={auto}
        aria-label="Meeting title"
        className="-mx-1 w-full rounded-md bg-transparent px-1 text-xl font-semibold tracking-tight ring-1 ring-ring outline-none placeholder:text-muted-foreground/60"
        data-testid="meeting-title-edit"
      />
    );
  }
  return (
    <h1 className="text-xl font-semibold tracking-tight" data-testid="meeting-heading">
      <button
        type="button"
        onClick={() => {
          cancelled.current = false;
          setDraft(meeting.title || "");
        }}
        title="Rename"
        className="group -mx-1 inline-flex items-center gap-2 rounded-md px-1 text-left hover:bg-muted"
        data-testid="rename-meeting"
      >
        {meetingTitle(meeting).text}
        <Pencil
          className="size-3.5 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
          aria-hidden
        />
      </button>
    </h1>
  );
}
