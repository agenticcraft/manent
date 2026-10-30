import { Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { formatDuration } from "@/lib/format";

/** Quiet audio player: play/pause, seek bar and time. */
export function AudioPlayer({
  src,
  fallbackDurationMs,
}: {
  src: string;
  fallbackDurationMs: number;
}) {
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(fallbackDurationMs / 1000);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const el = audio.current;
    if (!el) return;
    const onTime = () => setTime(el.currentTime);
    const onMeta = () => Number.isFinite(el.duration) && setDuration(el.duration);
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onError = () => setError("This recording can’t be played.");
    el.addEventListener("timeupdate", onTime);
    el.addEventListener("loadedmetadata", onMeta);
    el.addEventListener("durationchange", onMeta);
    el.addEventListener("play", onPlay);
    el.addEventListener("pause", onPause);
    el.addEventListener("ended", onPause);
    el.addEventListener("error", onError);
    return () => {
      el.removeEventListener("timeupdate", onTime);
      el.removeEventListener("loadedmetadata", onMeta);
      el.removeEventListener("durationchange", onMeta);
      el.removeEventListener("play", onPlay);
      el.removeEventListener("pause", onPause);
      el.removeEventListener("ended", onPause);
      el.removeEventListener("error", onError);
    };
  }, []);

  const toggle = () => {
    const el = audio.current;
    if (!el) return;
    if (el.paused) el.play().catch((e: unknown) => setError(String(e)));
    else el.pause();
  };

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
        className="h-1 flex-1 cursor-pointer accent-brand"
        data-testid="seek"
      />
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
