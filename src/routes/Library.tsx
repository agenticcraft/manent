import { FileAudio, Mic, NotebookText, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { EmptyState } from "@/components/EmptyState";
import { JobsPanel } from "@/components/JobsPanel";
import { MeetingRow } from "@/components/MeetingRow";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatDateTime, formatDuration, meetingTitle } from "@/lib/format";
import { call, on, type JobView, type MeetingSummary, type SearchHit } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** What "Import audio" offers (decision-3: what symphonia reads). */
const IMPORT_EXTENSIONS = [
  "wav",
  "mp3",
  "m4a",
  "mp4",
  "aac",
  "mov",
  "flac",
  "ogg",
  "oga",
  "mkv",
  "webm",
];

/** Library (doc-3 screen 1): searchable list of meetings, newest first. */
export function Library({
  onNewRecording,
  onOpenMeeting,
  recording,
}: {
  onNewRecording: () => Promise<void>;
  /** Open a meeting, optionally at a moment (ms) of its transcript. */
  onOpenMeeting: (id: number, atMs?: number) => void;
  /** A recording is running: starting another one is not possible. */
  recording: boolean;
}) {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [starting, setStarting] = useState(false);

  const [jobs, setJobs] = useState<JobView[]>([]);
  const [hits, setHits] = useState<SearchHit[]>([]);

  // Transcript search (VRT-20), a moment after typing stops.
  useEffect(() => {
    const q = query.trim();
    if (q.length < 2) return;
    const timer = window.setTimeout(() => {
      call("search_transcripts", { query: q }).then(setHits, (e: unknown) => setError(String(e)));
    }, 200);
    return () => window.clearTimeout(timer);
  }, [query]);

  const load = useCallback(() => {
    call("list_meetings").then(setMeetings, (e: unknown) => setError(String(e)));
  }, []);
  useEffect(load, [load]);

  // Background imports and transcriptions: the list follows them (new meetings, statuses).
  useEffect(() => {
    call("list_jobs").then(setJobs, () => {});
    const off = on("jobs:changed", (list) => {
      setJobs(list);
      load();
    });
    return () => {
      off.then((f) => f());
    };
  }, [load]);

  const importFiles = async () => {
    setError(null);
    try {
      const picked = await open({
        multiple: true,
        title: "Import recordings",
        filters: [{ name: "Audio and video", extensions: IMPORT_EXTENSIONS }],
      });
      if (picked && picked.length > 0) await call("import_audio", { paths: picked });
    } catch (e) {
      setError(String(e));
    }
  };

  const shownHits = query.trim().length >= 2 ? hits : [];

  const visible = useMemo(() => {
    const q = query.trim().toLocaleLowerCase();
    if (!meetings || !q) return meetings ?? [];
    return meetings.filter((m) => meetingTitle(m).text.toLocaleLowerCase().includes(q));
  }, [meetings, query]);

  const remove = async (id: number) => {
    try {
      await call("delete_meeting", { id });
      setMeetings((ms) => ms?.filter((m) => m.id !== id) ?? null);
    } catch (e) {
      setError(String(e));
    }
  };

  const actions = (
    <>
      <Button variant="ghost" size="sm" onClick={importFiles} data-testid="import-audio">
        <FileAudio /> Import audio
      </Button>
      <Button
        size="sm"
        className="bg-brand text-brand-foreground hover:bg-brand/90"
        disabled={recording || starting}
        onClick={async () => {
          setStarting(true);
          setError(null);
          try {
            await onNewRecording();
          } catch (e) {
            setError(String(e));
          } finally {
            setStarting(false);
          }
        }}
        data-testid="new-recording"
      >
        <Mic /> New recording
      </Button>
    </>
  );

  return (
    <>
      <PageHeader title="Library" actions={actions} />
      {error && (
        <p role="alert" className="mb-4 text-sm text-destructive">
          {error}
        </p>
      )}
      <JobsPanel jobs={jobs} onOpenMeeting={onOpenMeeting} />
      {meetings && meetings.length === 0 && (
        <EmptyState icon={NotebookText} title="No meetings yet">
          Record a meeting, or import an audio or video file (or drop it here). Transcripts in Dutch
          and English appear here.
        </EmptyState>
      )}
      {meetings && meetings.length > 0 && (
        <div>
          <div className="relative mb-2">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <Input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search meetings and transcripts"
              aria-label="Search meeting titles and transcripts"
              className="pl-8"
              data-testid="meeting-search"
            />
          </div>
          {visible.length > 0 && (
            <ul className="divide-y divide-border" aria-label="Meetings">
              {visible.map((m) => (
                <MeetingRow key={m.id} meeting={m} onOpen={onOpenMeeting} onDelete={remove} />
              ))}
            </ul>
          )}
          {shownHits.length > 0 && (
            <TranscriptHits hits={shownHits} meetings={meetings} onOpen={onOpenMeeting} />
          )}
          {visible.length === 0 && shownHits.length === 0 && (
            <p className="py-12 text-center text-sm text-muted-foreground" data-testid="no-match">
              No meetings match “{query.trim()}”.
            </p>
          )}
        </div>
      )}
    </>
  );
}

const MATCH_START = String.fromCharCode(1);
const MATCH_END = String.fromCharCode(2);

/** Snippet text with the matched words (wrapped in U+0001/U+0002) highlighted. */
function Snippet({ text }: { text: string }) {
  const parts: { text: string; match: boolean }[] = [];
  let rest = text;
  while (rest.length > 0) {
    const start = rest.indexOf(MATCH_START);
    const end = start >= 0 ? rest.indexOf(MATCH_END, start) : -1;
    if (start < 0 || end < 0) {
      parts.push({ text: rest, match: false });
      break;
    }
    if (start > 0) parts.push({ text: rest.slice(0, start), match: false });
    parts.push({ text: rest.slice(start + 1, end), match: true });
    rest = rest.slice(end + 1);
  }
  return (
    <>
      {parts.map((p, i) =>
        p.match ? (
          <mark key={i} className="rounded-sm bg-brand/20 px-0.5 text-foreground">
            {p.text}
          </mark>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </>
  );
}

/** Transcript matches grouped per meeting; a match opens the meeting at that moment. */
function TranscriptHits({
  hits,
  meetings,
  onOpen,
}: {
  hits: SearchHit[];
  meetings: MeetingSummary[];
  onOpen: (id: number, atMs?: number) => void;
}) {
  const groups = new Map<number, SearchHit[]>();
  for (const h of hits) groups.set(h.meeting_id, [...(groups.get(h.meeting_id) ?? []), h]);
  return (
    <section className="mt-6" aria-label="In transcripts" data-testid="transcript-hits">
      <h2 className="mb-2 text-xs font-medium tracking-wide text-muted-foreground uppercase">
        In transcripts
      </h2>
      <ul className="flex flex-col gap-3">
        {[...groups].map(([meetingId, list]) => {
          const m = meetings.find((x) => x.id === meetingId);
          return (
            <li key={meetingId} className="rounded-lg border px-3 py-2">
              {m ? (
                <p className="flex items-baseline justify-between gap-3 text-sm">
                  <span
                    className={cn(
                      "truncate font-medium",
                      meetingTitle(m).auto && "text-foreground/70",
                    )}
                  >
                    {meetingTitle(m).text}
                  </span>
                  <span
                    className="shrink-0 text-xs text-muted-foreground tabular-nums"
                    data-testid="hit-meeting-date"
                  >
                    {formatDateTime(m.started_at)} · {formatDuration(m.duration_ms)}
                  </span>
                </p>
              ) : (
                <p className="text-sm font-medium">Meeting</p>
              )}
              <ul className="mt-1 flex flex-col">
                {list.slice(0, 3).map((h) => (
                  <li key={h.segment_id}>
                    <button
                      type="button"
                      className="flex w-full gap-3 rounded-md px-1 py-1 text-left text-sm hover:bg-muted"
                      onClick={() => onOpen(meetingId, h.start_ms)}
                      data-testid="transcript-hit"
                      data-meeting-id={meetingId}
                      data-start-ms={h.start_ms}
                    >
                      <span className="w-10 shrink-0 text-xs leading-5 text-muted-foreground tabular-nums">
                        {formatDuration(h.start_ms)}
                      </span>
                      <span className="text-muted-foreground">
                        <Snippet text={h.snippet} />
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
