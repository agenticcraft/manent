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
  /** Live captions are on (the engine and model are downloaded). */
  captions: boolean;
}

/** A recognised word; times are milliseconds on the track (Rust: engine::transcript::Word). */
export interface Word {
  text: string;
  start_ms: number;
  end_ms: number;
  conf: number;
}

/** One transcribed live chunk of one track (Rust: live::Caption). */
export interface Caption {
  meeting_id: number;
  track_id: number;
  source_id: string;
  kind: SourceKind;
  start_ms: number;
  end_ms: number;
  text: string;
  words: Word[];
}

/**
 * A transcript segment: a live caption (no label) or a speaker block from the rolling or
 * final pass (Rust: commands::TranscriptSegment).
 */
export interface TranscriptSegment {
  meeting_id: number;
  track_id: number;
  kind: SourceKind;
  /** "me", "1", "2", ... ; null for live captions. */
  label: string | null;
  pass: "caption" | "rolling" | "final";
  start_ms: number;
  end_ms: number;
  text: string;
  words: Word[];
}

/** A pass replaced the segments that start in [from_ms, to_ms) (Rust: commands::PassEvent). */
export interface PassEvent {
  meeting_id: number;
  from_ms: number;
  to_ms: number;
  segments: TranscriptSegment[];
}

/** A meeting's speaker label: who it is or might be (Rust: commands::VoiceView). */
export interface VoiceView {
  label: string;
  speaker: Speaker | null;
  /** Suggested by voice (the user confirms). */
  suggestion: Speaker | null;
  distance: number | null;
  clean_seconds: number;
  confirmed: boolean;
}

/** Transcript export formats (Rust: export::Format). */
export type ExportFormat = "md" | "txt" | "srt" | "json";

/** A transcript match (Rust: db::SearchHit). */
export interface SearchHit {
  meeting_id: number;
  segment_id: number;
  start_ms: number;
  /** The matching words are wrapped in U+0001 and U+0002. */
  snippet: string;
}

/** A background transcription job (Rust: jobs::JobView). */
export interface JobView {
  id: number;
  kind: "import" | "transcribe";
  /** File or meeting name. */
  name: string;
  meeting_id: number | null;
  stage: "waiting" | "decoding" | "transcribing" | "done" | "failed";
  /** 0..1 (transcription is an estimate from the audio length). */
  progress: number;
  error: string | null;
}

/** A known person on the Speakers screen (Rust: db::Person). */
export interface Person {
  id: number;
  name: string;
  color: number;
  meetings: number;
  samples: number;
}

/** A caption could not be made; the recording goes on (Rust: commands::CaptionError). */
export interface CaptionError {
  meeting_id: number;
  message: string;
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

/** A running engine download (Rust: engine::DownloadProgress). */
export interface DownloadProgress {
  item: "engine" | "model" | "notice";
  received: number;
  total: number;
}

export type HelperState = "stopped" | "starting" | "ready" | "restarting" | "failed";

/** The engine helper's ready report (Rust: engine::helper::ReadyInfo). */
export interface EngineInfo {
  abi: number;
  load_ms: number;
  backends: number;
  components: string[];
}

/** Engine and model state, Settings > Transcription (Rust: engine::EngineStatus). */
export interface EngineStatus {
  /** A prebuilt engine exists for this platform. */
  supported: boolean;
  engine_installed: boolean;
  model_installed: boolean;
  /** Bytes still to download. */
  download_size: number;
  download: DownloadProgress | null;
  download_error: string | null;
  helper: HelperState;
  helper_info: EngineInfo | null;
  helper_pid: number | null;
  helper_error: string | null;
  /** Automatic restarts after a crash since the app started. */
  restarts: number;
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
  live_transcript: { args: { meetingId: number }; result: TranscriptSegment[] };
  get_me_name: { args: NoArgs; result: string };
  meeting_voices: { args: { meetingId: number }; result: VoiceView[] };
  name_speaker: { args: { meetingId: number; label: string; name: string }; result: null };
  confirm_suggestion: { args: { meetingId: number; label: string }; result: null };
  merge_labels: { args: { meetingId: number; from: string; into: string }; result: null };
  list_people: { args: NoArgs; result: Person[] };
  import_audio: { args: { paths: string[] }; result: number[] };
  meeting_waveform: { args: { id: number }; result: number[] };
  export_meeting: {
    args: { id: number; format: ExportFormat; path: string; dateLabel: string };
    result: null;
  };
  search_transcripts: { args: { query: string }; result: SearchHit[] };
  retranscribe: { args: { meetingId: number }; result: number };
  list_jobs: { args: NoArgs; result: JobView[] };
  dismiss_job: { args: { id: number }; result: null };
  rename_person: { args: { id: number; name: string }; result: null };
  merge_people: { args: { keep: number; merged: number }; result: null };
  delete_person: { args: { id: number }; result: null };
  set_me_name: { args: { name: string }; result: null };
  engine_status: { args: NoArgs; result: EngineStatus };
  engine_download: { args: NoArgs; result: null };
  engine_cancel_download: { args: NoArgs; result: null };
  engine_start: { args: NoArgs; result: EngineStatus };
  engine_notice: { args: NoArgs; result: string | null };
}

/** Event name -> payload type. */
export interface Events {
  "audio:level": LevelEvent;
  /** Test meters in Settings > Sources. */
  "audio:monitor-level": LevelEvent;
  /** Every change of the engine, model download and helper state. */
  "engine:status": EngineStatus;
  /** A live caption chunk (VRT-16). */
  "transcript:caption": Caption;
  "transcript:error": CaptionError;
  /** A rolling pass replaced a stretch with speaker blocks (VRT-17). */
  "transcript:pass": PassEvent;
  /** The final pass replaced the whole meeting's transcript. */
  "transcript:final": PassEvent;
  /** Who the speakers are changed; meeting_id null = people in general (VRT-18). */
  "speakers:changed": { meeting_id: number | null };
  /** The job list changed (VRT-19). */
  "jobs:changed": JobView[];
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
