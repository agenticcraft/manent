import { Bookmark, Headset, Pause, Play, Square, Users } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { LiveTranscript } from "@/components/LiveTranscript";
import { SourceChip } from "@/components/SourceChip";
import { Button } from "@/components/ui/button";
import { formatDuration, meetingTitle } from "@/lib/format";
import { call, on, type Mark, type RecordingStarted } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** Live recording (doc-3 screen 2). Captions from VRT-16; speakers come with VRT-17. */
export function Live({
  session,
  onStopped,
}: {
  session: RecordingStarted;
  onStopped: (meetingId: number) => void;
}) {
  const [title, setTitle] = useState("");
  const savedTitle = useRef("");
  const titleNow = useRef("");
  useEffect(() => {
    titleNow.current = title;
  }, [title]);

  // Stopped from the tray while a title was being typed: keep it (the meeting still exists).
  useEffect(
    () => () => {
      const t = titleNow.current.trim();
      if (t !== savedTitle.current) call("rename_meeting", { id: session.meeting_id, title: t });
    },
    [session.meeting_id],
  );
  const [paused, setPaused] = useState(false);
  const [pausedMs, setPausedMs] = useState(0);
  const [pausedAt, setPausedAt] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [levels, setLevels] = useState<Record<string, number>>({});
  const [muted, setMuted] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState(false);
  // A call (only the user on the mic) or a room (everyone there on the mic), VRT-47.
  const [inRoom, setInRoom] = useState(session.in_room);
  // Marked moments (VRT-52), from the Mark button or the global shortcut.
  const [marks, setMarks] = useState<Mark[]>([]);
  const [justMarked, setJustMarked] = useState<Mark | null>(null);
  useEffect(() => {
    const id = session.meeting_id;
    const load = () => call("list_marks", { meetingId: id }).then(setMarks, () => {});
    load();
    const off = on("meeting:marks", (e) => {
      if (e.meeting_id !== id) return;
      const added = e.added;
      if (added) {
        setMarks((cur) =>
          cur.some((m) => m.id === added.id)
            ? cur
            : [...cur, added].sort((a, b) => a.at_ms - b.at_ms),
        );
        setJustMarked(added);
      } else load();
    });
    return () => {
      off.then((f) => f());
    };
  }, [session.meeting_id]);
  useEffect(() => {
    if (!justMarked) return;
    const t = window.setTimeout(() => setJustMarked(null), 2000);
    return () => window.clearTimeout(t);
  }, [justMarked]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    const unlisten = on("audio:level", (e) =>
      setLevels((l) => (l[e.source_id] === e.rms_db ? l : { ...l, [e.source_id]: e.rms_db })),
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const elapsed = now - session.started_at - pausedMs - (pausedAt !== null ? now - pausedAt : 0);

  const run = async (f: () => Promise<unknown>) => {
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    }
  };

  const saveTitle = () =>
    run(async () => {
      if (title.trim() === savedTitle.current) return;
      await call("rename_meeting", { id: session.meeting_id, title });
      savedTitle.current = title.trim();
    });

  const togglePause = () =>
    run(async () => {
      if (paused) {
        await call("resume_recording");
        setPausedMs((p) => p + (Date.now() - (pausedAt ?? Date.now())));
        setPausedAt(null);
      } else {
        await call("pause_recording");
        setPausedAt(Date.now());
      }
      setPaused(!paused);
    });

  const toggleMute = (sourceId: string) =>
    run(async () => {
      const next = !muted[sourceId];
      await call("set_source_muted", { sourceId, muted: next });
      setMuted((m) => ({ ...m, [sourceId]: next }));
    });

  const mark = () =>
    run(async () => {
      await call("mark_moment");
    });

  const switchRoom = (next: boolean) =>
    run(async () => {
      if (next === inRoom) return;
      await call("set_recording_in_room", { inRoom: next });
      setInRoom(next);
    });

  const stop = () =>
    run(async () => {
      setBusy(true);
      try {
        await saveTitle();
        const stopped = await call("stop_recording");
        onStopped(stopped.meeting_id);
      } finally {
        setBusy(false);
      }
    });

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-3 pb-6">
        <span
          className={cn(
            "size-2.5 shrink-0 rounded-full",
            paused ? "bg-muted-foreground/50" : "recording-pulse bg-recording",
          )}
          aria-label={paused ? "Paused" : "Recording"}
          data-testid="recording-indicator"
        />
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={saveTitle}
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          placeholder={meetingTitle({ title: "", started_at: session.started_at }).text}
          aria-label="Meeting title"
          className="min-w-0 flex-1 bg-transparent text-xl font-semibold tracking-tight outline-none placeholder:text-muted-foreground/60"
          data-testid="meeting-title-input"
        />
        <span
          className="text-sm text-muted-foreground tabular-nums"
          data-testid="recording-timer"
          aria-label="Recording time"
        >
          {formatDuration(Math.max(0, elapsed))}
        </span>
        <Button
          variant="outline"
          size="sm"
          onClick={mark}
          title="Mark this moment"
          data-testid="mark-moment"
          data-marks={marks.length}
        >
          <Bookmark className={cn(justMarked && "fill-brand text-brand")} />
          <span className="tabular-nums">
            {justMarked ? `Marked ${formatDuration(justMarked.at_ms)}` : "Mark"}
          </span>
        </Button>
        <Button variant="outline" size="sm" onClick={togglePause} data-testid="pause-toggle">
          {paused ? <Play /> : <Pause />} {paused ? "Resume" : "Pause"}
        </Button>
        <Button size="sm" onClick={stop} disabled={busy} data-testid="stop">
          <Square className="fill-current" /> Stop
        </Button>
      </header>

      <div className="flex flex-wrap gap-2 pb-6">
        {session.sources.map((s) => (
          <SourceChip
            key={s.id}
            source={s}
            rmsDb={levels[s.id]}
            muted={!!muted[s.id]}
            onToggleMute={() => toggleMute(s.id)}
          />
        ))}
        <div
          role="radiogroup"
          aria-label="Who is on the microphone"
          className="ml-auto inline-flex items-center rounded-full border p-0.5 text-sm"
          data-testid="meeting-kind"
          data-in-room={inRoom}
        >
          {(
            [
              [false, "Call", Headset, "Only you are on the microphone"],
              [true, "In the room", Users, "Everyone in the room is on the microphone"],
            ] as const
          ).map(([value, label, Icon, hint]) => (
            <button
              key={label}
              type="button"
              role="radio"
              aria-checked={inRoom === value}
              title={hint}
              onClick={() => switchRoom(value)}
              className={cn(
                "flex items-center gap-1.5 rounded-full px-3 py-1 transition-colors",
                inRoom === value
                  ? "bg-muted font-medium text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
              data-testid={value ? "kind-room" : "kind-call"}
            >
              <Icon className="size-3.5" aria-hidden />
              {label}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <p role="alert" className="pb-4 text-sm text-destructive">
          {error}
        </p>
      )}

      <LiveTranscript session={session} paused={paused} inRoom={inRoom} marks={marks} />
    </div>
  );
}
