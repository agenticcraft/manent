/**
 * Typed IPC between the UI and the Rust core (AGENTS.md: the UI talks to Rust only through here).
 * Commands carry requests; events carry streams such as audio levels and transcript segments.
 * Keep these types in sync with the Rust side (src-tauri/src/commands.rs).
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
}

export type SourceKind = "mic" | "loopback";

/** A recordable source (Rust: audio::SourceInfo). */
export interface SourceInfo {
  id: string;
  name: string;
  kind: SourceKind;
  is_default: boolean;
}

export interface RecordingStarted {
  meeting_id: number;
  /** Unix epoch milliseconds. */
  started_at: number;
  sources: SourceInfo[];
}

/** What was written for one source when a recording stops (Rust: audio::TrackSummary). */
export interface TrackSummary {
  source_id: string;
  kind: SourceKind;
  file: string;
  seconds: number;
  dropped_samples: number;
}

/** Live level of one source, ~20 times per second (Rust: audio::recorder::LevelEvent). */
export interface LevelEvent {
  source_id: string;
  kind: SourceKind;
  rms_db: number;
  peak_db: number;
  paused: boolean;
  /** A muted source records silence; the level still shows the input. */
  muted: boolean;
}

export interface RecordingStopped {
  meeting_id: number;
  tracks: TrackSummary[];
  /** Path of the playback mixdown (mix.opus), or null if it could not be made. */
  mix: string | null;
}

export interface Speaker {
  id: number;
  name: string;
  /** Speaker color slot 1-8 (CSS --speaker-N). */
  color: number;
}

export type MeetingStatus = "recording" | "processing" | "done" | "failed";

/** A meeting in the Library (Rust: commands::MeetingSummary). */
export interface MeetingSummary {
  id: number;
  title: string;
  /** Unix epoch milliseconds. */
  started_at: number;
  duration_ms: number;
  status: MeetingStatus;
  dir: string;
  speakers: Speaker[];
}

/** Settings > Sources (Rust: commands::SourceSettings). */
export interface SourceSettings {
  available: SourceInfo[];
  mic: string | null;
  /** null = don't record system audio. */
  system: string | null;
}

/** A meeting with its playback file (Rust: commands::MeetingDetail). */
export interface MeetingDetail {
  id: number;
  title: string;
  started_at: number;
  duration_ms: number;
  status: MeetingStatus;
  dir: string;
  /** Playback URL on the app's local media server (127.0.0.1), or null without a mix. */
  mix_url: string | null;
}

type NoArgs = Record<string, never>;

/** Command name -> argument and result types. */
export interface Commands {
  app_info: { args: NoArgs; result: AppInfo };
  list_sources: { args: NoArgs; result: SourceInfo[] };
  start_recording: { args: NoArgs; result: RecordingStarted };
  current_recording: { args: NoArgs; result: RecordingStarted | null };
  pause_recording: { args: NoArgs; result: null };
  resume_recording: { args: NoArgs; result: null };
  set_source_muted: { args: { sourceId: string; muted: boolean }; result: null };
  stop_recording: { args: NoArgs; result: RecordingStopped };
  rename_meeting: { args: { id: number; title: string }; result: null };
  get_recording_sources: { args: NoArgs; result: SourceSettings };
  set_recording_sources: { args: { mic: string; system: string | null }; result: null };
  start_source_monitor: { args: { sourceIds: string[] }; result: null };
  stop_source_monitor: { args: NoArgs; result: null };
  list_meetings: { args: NoArgs; result: MeetingSummary[] };
  delete_meeting: { args: { id: number }; result: boolean };
  get_meeting: { args: { id: number }; result: MeetingDetail | null };
}

/** Event name -> payload type. */
export interface Events {
  "audio:level": LevelEvent;
  /** Test meters in Settings > Sources. */
  "audio:monitor-level": LevelEvent;
}

type ArgsOf<K extends keyof Commands> = Commands[K]["args"] extends NoArgs
  ? []
  : [args: Commands[K]["args"]];

/** Call a Rust command with typed arguments and result. */
export function call<K extends keyof Commands>(
  command: K,
  ...args: ArgsOf<K>
): Promise<Commands[K]["result"]> {
  return invoke<Commands[K]["result"]>(command, args[0]);
}

/** Subscribe to a Rust event with a typed payload. Returns the unsubscribe function. */
export function on<K extends keyof Events & string>(
  event: K,
  handler: (payload: Events[K]) => void,
): Promise<UnlistenFn> {
  return listen<Events[K]>(event, (e) => handler(e.payload));
}
