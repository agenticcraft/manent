/** Display helpers shared by the screens. */

/** 1:02:03 for meetings over an hour, otherwise 2:03. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** Date and time in the user's locale, e.g. "29 Oct 2026, 14:05". */
export function formatDateTime(epochMs: number): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(epochMs),
  );
}

/**
 * A meeting's title, or for one without a title a name from when it was held, e.g. "Sunday
 * evening meeting" (VRT-42); `auto` tells which, so screens can show it more quietly.
 */
export function meetingTitle(m: { title: string; started_at: number }): {
  text: string;
  auto: boolean;
} {
  const title = m.title.trim();
  if (title) return { text: title, auto: false };
  const d = new Date(m.started_at);
  const day = new Intl.DateTimeFormat(undefined, { weekday: "long" }).format(d);
  const h = d.getHours();
  const part =
    h < 5 ? "night" : h < 12 ? "morning" : h < 17 ? "afternoon" : h < 22 ? "evening" : "night";
  return { text: `${day} ${part} meeting`, auto: true };
}

/** Up to two initials from a name: "Bram Jansen" -> "BJ", "anna" -> "A". */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  return parts
    .slice(0, 2)
    .map((p) => p[0]!.toUpperCase())
    .join("");
}

/** Static class names so Tailwind generates them (speaker color slots 1-8). */
const SPEAKER_BG = [
  "bg-speaker-1",
  "bg-speaker-2",
  "bg-speaker-3",
  "bg-speaker-4",
  "bg-speaker-5",
  "bg-speaker-6",
  "bg-speaker-7",
  "bg-speaker-8",
];

export function speakerBg(color: number): string {
  return SPEAKER_BG[(((color - 1) % 8) + 8) % 8]!;
}

/** Map an RMS level in dBFS to 0..1 for display (-60 dB and below is empty). */
export function meterFraction(rmsDb: number | undefined): number {
  if (rmsDb === undefined || !Number.isFinite(rmsDb)) return 0;
  return Math.min(1, Math.max(0, (rmsDb + 60) / 60));
}

/** Byte counts for download sizes: "14 MB", "1.1 GB". */
export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
  return `${Math.round(bytes / 1e3)} kB`;
}
