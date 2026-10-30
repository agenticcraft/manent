import { FileAudio, Mic, NotebookText, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { EmptyState } from "@/components/EmptyState";
import { MeetingRow } from "@/components/MeetingRow";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { call, type MeetingSummary } from "@/lib/ipc";

/** Library (doc-3 screen 1): searchable list of meetings, newest first. */
export function Library({
  onNewRecording,
  onOpenMeeting,
  recording,
}: {
  onNewRecording: () => Promise<void>;
  onOpenMeeting: (id: number) => void;
  /** A recording is running: starting another one is not possible. */
  recording: boolean;
}) {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [starting, setStarting] = useState(false);

  const load = useCallback(() => {
    call("list_meetings").then(setMeetings, (e: unknown) => setError(String(e)));
  }, []);
  useEffect(load, [load]);

  const visible = useMemo(() => {
    const q = query.trim().toLocaleLowerCase();
    if (!meetings || !q) return meetings ?? [];
    return meetings.filter((m) => (m.title || "Untitled meeting").toLocaleLowerCase().includes(q));
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
      <Button variant="ghost" size="sm" disabled>
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
      {meetings && meetings.length === 0 && (
        <EmptyState icon={NotebookText} title="No meetings yet">
          Record a meeting or import an audio file. Transcripts in Dutch and English appear here.
        </EmptyState>
      )}
      {meetings && meetings.length > 0 && (
        <div className="max-w-3xl">
          <div className="relative mb-2">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <Input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search meetings"
              aria-label="Search meetings by title"
              className="pl-8"
              data-testid="meeting-search"
            />
          </div>
          {visible.length > 0 ? (
            <ul className="divide-y divide-border" aria-label="Meetings">
              {visible.map((m) => (
                <MeetingRow key={m.id} meeting={m} onOpen={onOpenMeeting} onDelete={remove} />
              ))}
            </ul>
          ) : (
            <p className="py-12 text-center text-sm text-muted-foreground" data-testid="no-match">
              No meetings match “{query.trim()}”.
            </p>
          )}
        </div>
      )}
    </>
  );
}
