import { Cloud, CloudAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { formatDateTime } from "@/lib/format";
import { call, on, type SyncStatus } from "@/lib/ipc";

/** One quiet line in the Library while Nextcloud sync is on (VRT-39): last sync or the error. */
export function SyncLine() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  useEffect(() => {
    call("sync_status").then(setStatus, () => {});
    const off = on("sync:changed", setStatus);
    return () => {
      off.then((f) => f());
    };
  }, []);
  if (!status || status.server === null || status.state === "connecting") return null;
  const failed = status.state === "error";
  return (
    <p
      className={
        failed
          ? "mb-3 flex items-center gap-1.5 text-xs text-destructive"
          : "mb-3 flex items-center gap-1.5 text-xs text-muted-foreground"
      }
      data-testid="sync-line"
      role={failed ? "alert" : undefined}
    >
      {failed ? (
        <CloudAlert className="size-3.5" aria-hidden />
      ) : (
        <Cloud className="size-3.5" aria-hidden />
      )}
      {failed
        ? `Nextcloud sync failed: ${status.error ?? "unknown error"}`
        : status.state === "syncing"
          ? "Syncing with Nextcloud…"
          : status.last_sync
            ? `Synced with Nextcloud ${formatDateTime(status.last_sync)}`
            : "Connected to Nextcloud"}
    </p>
  );
}
