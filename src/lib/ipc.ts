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
  meeting_id: string;
  dir: string;
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
}

type NoArgs = Record<string, never>;

/** Command name -> argument and result types. */
export interface Commands {
  app_info: { args: NoArgs; result: AppInfo };
  list_sources: { args: NoArgs; result: SourceInfo[] };
  start_recording: { args: { sourceIds: string[] }; result: RecordingStarted };
  pause_recording: { args: NoArgs; result: null };
  resume_recording: { args: NoArgs; result: null };
  stop_recording: { args: NoArgs; result: TrackSummary[] };
}

/** Event name -> payload type. */
export interface Events {
  "audio:level": LevelEvent;
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
