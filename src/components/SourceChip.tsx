import { Mic, MicOff, Speaker, VolumeX } from "lucide-react";
import { LevelMeter } from "@/components/LevelMeter";
import type { SourceInfo } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** A recorded source in the Live top bar: icon, name, mini level meter and mute toggle. */
export function SourceChip({
  source,
  rmsDb,
  muted,
  onToggleMute,
}: {
  source: SourceInfo;
  rmsDb?: number;
  muted: boolean;
  onToggleMute: () => void;
}) {
  const isMic = source.kind === "mic";
  const Icon = muted ? (isMic ? MicOff : VolumeX) : isMic ? Mic : Speaker;
  const label = isMic ? "Microphone" : "System audio";
  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-full border py-1 pr-1 pl-3 text-xs",
        muted && "text-muted-foreground",
      )}
      data-testid="source-chip"
      data-source-id={source.id}
      data-muted={muted}
      title={source.name}
    >
      <span className="max-w-40 truncate">{label}</span>
      <LevelMeter rmsDb={rmsDb} muted={muted} />
      <button
        type="button"
        onClick={onToggleMute}
        aria-pressed={muted}
        aria-label={`${muted ? "Unmute" : "Mute"} ${label.toLowerCase()}`}
        className="flex size-6 items-center justify-center rounded-full hover:bg-accent"
        data-testid="mute-toggle"
      >
        <Icon className="size-3.5" />
      </button>
    </div>
  );
}
