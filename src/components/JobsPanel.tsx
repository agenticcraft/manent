import { X } from "lucide-react";
import type { JobView } from "@/lib/ipc";
import { call } from "@/lib/ipc";
import { cn } from "@/lib/utils";

const STAGE = {
  waiting: "Waiting",
  decoding: "Reading the audio",
  transcribing: "Transcribing",
  done: "Done",
  failed: "Failed",
} as const;

/** Imports and transcriptions running in the background (VRT-19), above the meeting list. */
export function JobsPanel({
  jobs,
  onOpenMeeting,
}: {
  jobs: JobView[];
  onOpenMeeting: (id: number) => void;
}) {
  if (jobs.length === 0) return null;
  return (
    <ul className="mb-4 flex flex-col gap-2" aria-label="Transcription jobs" data-testid="jobs">
      {jobs.map((j) => {
        const finished = j.stage === "done" || j.stage === "failed";
        return (
          <li
            key={j.id}
            className="flex flex-col gap-1.5 rounded-lg border px-3 py-2"
            data-testid="job"
            data-stage={j.stage}
            data-job-id={j.id}
          >
            <div className="flex items-center gap-3 text-sm">
              <span className="min-w-0 flex-1 truncate">
                {j.kind === "import"
                  ? "Importing "
                  : j.kind === "recover"
                    ? "Recovering "
                    : "Transcribing "}
                {j.meeting_id !== null && j.stage === "done" ? (
                  <button
                    type="button"
                    className="font-medium underline-offset-2 hover:underline"
                    onClick={() => onOpenMeeting(j.meeting_id!)}
                  >
                    {j.name}
                  </button>
                ) : (
                  <span className="font-medium">{j.name}</span>
                )}
              </span>
              <span
                className={cn(
                  "text-xs tabular-nums",
                  j.stage === "failed" ? "text-destructive" : "text-muted-foreground",
                )}
              >
                {STAGE[j.stage]}
                {!finished && j.stage !== "waiting" && ` · ${Math.round(j.progress * 100)}%`}
              </span>
              {finished && (
                <button
                  type="button"
                  aria-label="Dismiss"
                  className="rounded p-0.5 text-muted-foreground hover:bg-muted"
                  onClick={() => call("dismiss_job", { id: j.id })}
                  data-testid="dismiss-job"
                >
                  <X className="size-3.5" />
                </button>
              )}
            </div>
            {!finished && (
              <div
                className="h-1 overflow-hidden rounded-full bg-muted"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(j.progress * 100)}
              >
                <div
                  className="h-full rounded-full bg-brand transition-[width] duration-500"
                  style={{ width: `${j.progress * 100}%` }}
                />
              </div>
            )}
            {j.error && <p className="text-xs text-destructive">{j.error}</p>}
          </li>
        );
      })}
    </ul>
  );
}
