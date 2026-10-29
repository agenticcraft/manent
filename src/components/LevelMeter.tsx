import { meterFraction } from "@/lib/format";
import { cn } from "@/lib/utils";

/** A slim horizontal level bar. */
export function LevelMeter({ rmsDb, muted }: { rmsDb?: number; muted?: boolean }) {
  const fraction = meterFraction(rmsDb);
  return (
    <div
      className="h-1.5 w-16 overflow-hidden rounded-full bg-muted"
      role="meter"
      aria-label="Input level"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(fraction * 100)}
      data-testid="level-meter"
    >
      <div
        className={cn(
          "h-full rounded-full transition-[width] duration-75",
          muted ? "bg-muted-foreground/40" : "bg-brand",
        )}
        style={{ width: `${fraction * 100}%` }}
      />
    </div>
  );
}
