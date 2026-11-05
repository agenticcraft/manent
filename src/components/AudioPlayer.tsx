import { Pause, Play } from "lucide-react";
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import { Button } from "@/components/ui/button";
import { formatDuration } from "@/lib/format";

/** What a parent can do with the player (the transcript seeks it, VRT-20). */
export interface PlayerHandle {
  /** Jump to a time (ms) and keep playing or paused as before; `play` starts playback. */
  seek: (ms: number, play?: boolean) => void;
}

/**
 * Quiet audio player (doc-3): play/pause, time, and a waveform to seek on. The waveform is
 * drawn under a transparent range input, so mouse, touch and keyboard seeking stay native.
 */
export function AudioPlayer({
  src,
  fallbackDurationMs,
  waveform,
  marks = [],
  onTime,
  ref,
}: {
  src: string;
  fallbackDurationMs: number;
  /** Loudness bars (0..1) over the whole meeting. */
  waveform?: number[];
  /** Marked moments (ms, VRT-52), shown as ticks above the waveform. */
  marks?: number[];
  /** The playback position in ms, about 4 times a second and on every seek. */
  onTime?: (ms: number) => void;
  ref?: Ref<PlayerHandle>;
}) {
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(fallbackDurationMs / 1000);
  const [error, setError] = useState<string | null>(null);
  const onTimeRef = useRef(onTime);
  /** A seek asked for before the audio knew its length; applied once it does. */
  const pending = useRef<number | null>(null);
  useEffect(() => {
    onTimeRef.current = onTime;
  }, [onTime]);

  useEffect(() => {
    const el = audio.current;
    if (!el) return;
    const report = () => {
      setTime(el.currentTime);
      onTimeRef.current?.(el.currentTime * 1000);
    };
    // WebKitGTK sometimes reports a duration of 0 for a short Ogg file that is fine (seen in
    // VRT-41; the same file is fine the next time). Keep the meeting's own length meanwhile
    // and load the file once more, which gives the real duration.
    let reloaded = false;
    const onMeta = () => {
      if (Number.isFinite(el.duration) && el.duration > 0) setDuration(el.duration);
      else if (el.duration === 0 && el.readyState >= 1 && !reloaded) {
        reloaded = true;
        el.load();
      }
    };
    // WebKit can ignore or reset a position set while the file is still loading: apply a
    // requested seek again once it can play, if it didn't stick.
    const applyPending = () => {
      if (pending.current === null || el.readyState < 1) return;
      if (Math.abs(el.currentTime * 1000 - pending.current) > 250) {
        el.currentTime = pending.current / 1000;
      }
      if (el.readyState >= 3) pending.current = null;
    };
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onError = () => setError("This recording can’t be played.");
    el.addEventListener("timeupdate", report);
    el.addEventListener("seeked", report);
    el.addEventListener("loadedmetadata", onMeta);
    el.addEventListener("loadedmetadata", applyPending);
    el.addEventListener("canplay", applyPending);
    el.addEventListener("canplay", onMeta);
    el.addEventListener("durationchange", onMeta);
    el.addEventListener("play", onPlay);
    el.addEventListener("pause", onPause);
    el.addEventListener("ended", onPause);
    el.addEventListener("error", onError);
    return () => {
      el.removeEventListener("timeupdate", report);
      el.removeEventListener("seeked", report);
      el.removeEventListener("loadedmetadata", onMeta);
      el.removeEventListener("loadedmetadata", applyPending);
      el.removeEventListener("canplay", applyPending);
      el.removeEventListener("canplay", onMeta);
      el.removeEventListener("durationchange", onMeta);
      el.removeEventListener("play", onPlay);
      el.removeEventListener("pause", onPause);
      el.removeEventListener("ended", onPause);
      el.removeEventListener("error", onError);
    };
  }, []);

  useImperativeHandle(ref, () => ({
    seek: (ms, play) => {
      const el = audio.current;
      if (!el) return;
      // Until the file can play, WebKit may drop a new position: remember it (see above).
      if (el.readyState < 3) pending.current = ms;
      if (el.readyState >= 1) el.currentTime = ms / 1000;
      setTime(ms / 1000);
      if (play && el.paused) el.play().catch((e: unknown) => setError(String(e)));
    },
  }));

  const toggle = () => {
    const el = audio.current;
    if (!el) return;
    if (el.paused) el.play().catch((e: unknown) => setError(String(e)));
    else el.pause();
  };

  // Space plays and pauses (VRT-42), unless typing or a control has the focus (a focused
  // button already handles Space itself).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code !== "Space" || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
      const t = e.target as HTMLElement | null;
      if (t?.closest("input, textarea, select, button, [contenteditable=true], [role=menu]"))
        return;
      const el = audio.current;
      if (!el) return;
      e.preventDefault();
      if (el.paused) el.play().catch((err: unknown) => setError(String(err)));
      else el.pause();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const fraction = duration > 0 ? Math.min(1, time / duration) : 0;

  return (
    <div className="flex items-center gap-3 rounded-lg border px-3 py-2" data-testid="player">
      {/* preload="auto": with "metadata" WebKitGTK never learns an Ogg file's duration and
          can't seek; mix files are small (~14 MB per hour). */}
      <audio ref={audio} src={src} preload="auto" data-testid="audio" />
      <Button
        size="icon"
        variant="ghost"
        onClick={toggle}
        aria-label={playing ? "Pause" : "Play"}
        data-testid="play-toggle"
      >
        {playing ? <Pause /> : <Play />}
      </Button>
      <span className="w-12 text-xs text-muted-foreground tabular-nums" data-testid="player-time">
        {formatDuration(Math.floor(time) * 1000)}
      </span>
      <div className="relative flex h-10 flex-1 items-center">
        {waveform && waveform.length > 0 ? (
          // One SVG that scales to any width; the played part is the same bars in the accent.
          <svg
            className="pointer-events-none absolute inset-0 h-full w-full"
            viewBox={`0 0 ${waveform.length} 100`}
            preserveAspectRatio="none"
            aria-hidden
            data-testid="waveform"
            data-bars={waveform.length}
          >
            <defs>
              <clipPath id="played">
                <rect x="0" y="0" width={fraction * waveform.length} height="100" />
              </clipPath>
            </defs>
            {[false, true].map((played) => (
              <g
                key={String(played)}
                className={played ? "fill-brand" : "fill-muted-foreground/30"}
                clipPath={played ? "url(#played)" : undefined}
              >
                {waveform.map((v, i) => {
                  const h = Math.max(6, v * 100);
                  return <rect key={i} x={i + 0.15} y={(100 - h) / 2} width={0.7} height={h} />;
                })}
              </g>
            ))}
          </svg>
        ) : (
          <div className="pointer-events-none absolute inset-x-0 h-1 rounded-full bg-muted">
            <div className="h-full rounded-full bg-brand" style={{ width: `${fraction * 100}%` }} />
          </div>
        )}
        {duration > 0 &&
          marks.map((ms) => (
            <span
              key={ms}
              className="pointer-events-none absolute top-0 h-2 w-0.5 -translate-x-1/2 rounded-full bg-brand"
              style={{ left: `${Math.min(100, (ms / 1000 / duration) * 100)}%` }}
              aria-hidden
              data-testid="player-mark"
            />
          ))}
        <input
          type="range"
          min={0}
          max={duration || 0}
          step={0.1}
          value={Math.min(time, duration || 0)}
          onChange={(e) => {
            const el = audio.current;
            if (el) el.currentTime = Number(e.target.value);
            setTime(Number(e.target.value));
          }}
          aria-label="Seek"
          className="relative h-full w-full cursor-pointer opacity-0"
          data-testid="seek"
        />
      </div>
      <span className="w-12 text-right text-xs text-muted-foreground tabular-nums">
        {formatDuration(duration * 1000)}
      </span>
      {error && (
        <span role="alert" className="text-xs text-destructive">
          {error}
        </span>
      )}
    </div>
  );
}
