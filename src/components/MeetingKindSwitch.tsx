import { Headset, Users } from "lucide-react";
import { cn } from "@/lib/utils";

const KINDS = [
  [false, "Call", Headset, "Only you are on the microphone"],
  [true, "In the room", Users, "Everyone in the room is on the microphone"],
] as const;

/** Who is on the microphone: a call or a room (VRT-47), on the record page and while recording. */
export function MeetingKindSwitch({
  inRoom,
  onChange,
  className,
}: {
  inRoom: boolean;
  onChange: (inRoom: boolean) => void;
  className?: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Who is on the microphone"
      className={cn("inline-flex items-center rounded-full border p-0.5 text-sm", className)}
      data-testid="meeting-kind"
      data-in-room={inRoom}
    >
      {KINDS.map(([value, label, Icon, hint]) => (
        <button
          key={label}
          type="button"
          role="radio"
          aria-checked={inRoom === value}
          title={hint}
          onClick={() => onChange(value)}
          className={cn(
            "flex items-center gap-1.5 rounded-full px-3 py-1 transition-colors",
            inRoom === value
              ? "bg-muted font-medium text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
          data-testid={value ? "kind-room" : "kind-call"}
        >
          <Icon className="size-3.5" aria-hidden />
          {label}
        </button>
      ))}
    </div>
  );
}
