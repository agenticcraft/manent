import { Pause, Play, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { LiveTranscript } from "@/components/LiveTranscript";
import { SourceChip } from "@/components/SourceChip";
import { Button } from "@/components/ui/button";
import { formatDuration } from "@/lib/format";
import { call, on, type RecordingStarted } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** Live recording (doc-3 screen 2). Captions from VRT-16; speakers come with VRT-17. */
export function Live({ session, onStopped }: { session: RecordingStarted; onStopped: () => void }) {
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

  const stop = () =>
    run(async () => {
      setBusy(true);
      try {
        await saveTitle();
        await call("stop_recording");
        onStopped();
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
          placeholder="Untitled meeting"
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
        <Button variant="outline" size="sm" onClick={togglePause} data-testid="pause-toggle">
          {paused ? <Play /> : <Pause />} {paused ? "Resume" : "Pause"}
        </Button>
        <Button size="sm" variant="secondary" onClick={stop} disabled={busy} data-testid="stop">
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
      </div>

      {error && (
        <p role="alert" className="pb-4 text-sm text-destructive">
          {error}
        </p>
      )}

      <LiveTranscript session={session} paused={paused} />
    </div>
  );
}
