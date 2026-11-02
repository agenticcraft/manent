import { Trash2 } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { formatDateTime, formatDuration, initials, meetingTitle, speakerBg } from "@/lib/format";
import type { MeetingSummary } from "@/lib/ipc";
import { cn } from "@/lib/utils";

const STATUS_LABEL: Record<MeetingSummary["status"], string | null> = {
  recording: "Recording",
  processing: "Processing",
  failed: "Failed",
  done: null,
};

/** One meeting in the Library: title, date, duration, speakers; delete with inline confirm. */
export function MeetingRow({
  meeting,
  onOpen,
  onDelete,
}: {
  meeting: MeetingSummary;
  onOpen: (id: number) => void;
  onDelete: (id: number) => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const status = STATUS_LABEL[meeting.status];
  const shown = meeting.speakers.slice(0, 4);
  const more = meeting.speakers.length - shown.length;

  return (
    <li
      className="group flex items-center gap-4 py-3.5"
      data-testid="meeting-row"
      data-meeting-id={meeting.id}
    >
      <button
        type="button"
        onClick={() => onOpen(meeting.id)}
        className="min-w-0 flex-1 rounded-md text-left outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        data-testid="open-meeting"
      >
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "truncate text-sm font-medium",
              meetingTitle(meeting).auto && "text-foreground/70",
            )}
            data-testid="meeting-title"
          >
            {meetingTitle(meeting).text}
          </span>
          {status && (
            <span
              className={cn(
                "rounded-full px-2 py-0.5 text-[11px] font-medium",
                meeting.status === "failed"
                  ? "bg-destructive/10 text-destructive"
                  : meeting.status === "recording"
                    ? "bg-recording/10 text-recording"
                    : "bg-muted text-muted-foreground",
              )}
            >
              {status}
            </span>
          )}
        </div>
        <div className="mt-0.5 text-xs text-muted-foreground tabular-nums">
          {formatDateTime(meeting.started_at)} · {formatDuration(meeting.duration_ms)}
        </div>
      </button>

      {shown.length > 0 && (
        <div className="flex -space-x-1" aria-label="Speakers">
          {shown.map((s) => (
            <span
              key={s.id}
              title={s.name}
              className={cn(
                "flex size-7 items-center justify-center rounded-full text-[11px] font-medium text-foreground ring-2 ring-background",
                speakerBg(s.color),
              )}
            >
              {initials(s.name)}
            </span>
          ))}
          {more > 0 && (
            <span className="flex size-7 items-center justify-center rounded-full bg-muted text-[11px] text-muted-foreground ring-2 ring-background">
              +{more}
            </span>
          )}
        </div>
      )}

      {confirming ? (
        <div className="flex items-center gap-2" role="group" aria-label="Confirm delete">
          <span className="text-xs text-muted-foreground">Delete meeting and audio?</span>
          <Button
            size="sm"
            variant="destructive"
            disabled={busy}
            data-testid="confirm-delete"
            onClick={async () => {
              setBusy(true);
              try {
                await onDelete(meeting.id);
              } finally {
                setBusy(false);
                setConfirming(false);
              }
            }}
          >
            Delete
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setConfirming(false)}>
            Cancel
          </Button>
        </div>
      ) : (
        <Button
          size="icon"
          variant="ghost"
          aria-label={`Delete ${meeting.title || "meeting"}`}
          data-testid="delete-meeting"
          className="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100"
          onClick={() => setConfirming(true)}
        >
          <Trash2 className="text-muted-foreground" />
        </Button>
      )}
    </li>
  );
}
