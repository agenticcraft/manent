import { Mic } from "lucide-react";
import { useEffect, useState } from "react";
import { MeetingKindSwitch } from "@/components/MeetingKindSwitch";
import { PageHeader } from "@/components/PageHeader";
import { SourcesSettings } from "@/components/SourcesSettings";
import { Button } from "@/components/ui/button";
import { call, type RecordingStarted } from "@/lib/ipc";

/**
 * The record page while nothing records (VRT-54): check the sources and their meters, choose a
 * call or a room, and start. Reaching this page never starts a recording.
 */
export function RecordReady({ onStarted }: { onStarted: (s: RecordingStarted) => void }) {
  const [inRoom, setInRoom] = useState<boolean | null>(null);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("get_in_room").then(setInRoom, (e: unknown) => setError(String(e)));
  }, []);

  const choose = async (next: boolean) => {
    setError(null);
    try {
      await call("set_recording_in_room", { inRoom: next });
      setInRoom(next);
    } catch (e) {
      setError(String(e));
    }
  };

  const start = async () => {
    setError(null);
    setStarting(true);
    try {
      onStarted(await call("start_recording"));
    } catch (e) {
      setError(String(e));
      setStarting(false);
    }
  };

  return (
    <div data-testid="record-ready">
      <PageHeader title="Record" />
      <div className="flex flex-col gap-6">
        <SourcesSettings />
        <div className="flex flex-wrap items-center justify-between gap-3 border-t pt-6">
          {inRoom !== null && <MeetingKindSwitch inRoom={inRoom} onChange={choose} />}
          <Button onClick={start} disabled={starting} data-testid="start-recording">
            <Mic /> {starting ? "Starting…" : "Start recording"}
          </Button>
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive" data-testid="record-error">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}
