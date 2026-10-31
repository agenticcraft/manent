import { useEffect, useState } from "react";
import { call, on, type TranscriptSegment, type VoiceView } from "@/lib/ipc";

/** Transcript segments in time order (then by track). */
export function sorted(list: TranscriptSegment[]): TranscriptSegment[] {
  return [...list].sort((a, b) => a.start_ms - b.start_ms || a.track_id - b.track_id);
}

/** Who a meeting's speakers are (reloaded on `speakers:changed`) and the user's own name. */
export function useSpeakers(meetingId: number, onError: (e: string) => void) {
  const [voices, setVoices] = useState<VoiceView[]>([]);
  const [meName, setMeName] = useState("Me");

  useEffect(() => {
    call("get_me_name").then(setMeName, () => {});
    const load = () => call("meeting_voices", { meetingId }).then(setVoices, () => {});
    load();
    const off = on("speakers:changed", (e) => {
      if (e.meeting_id === null || e.meeting_id === meetingId) load();
    });
    return () => {
      off.then((f) => f());
    };
  }, [meetingId]);

  const renameMe = async (name: string) => {
    const next = name.trim() || "Me";
    setMeName(next);
    try {
      await call("set_me_name", { name: next });
    } catch (e) {
      onError(String(e));
    }
  };

  return { voices, meName, renameMe };
}
