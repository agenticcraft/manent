import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatDateTime } from "@/lib/format";
import { call, on, type SyncStatus } from "@/lib/ipc";

/**
 * Settings > Nextcloud (VRT-39, decision-7): connect a folder on the user's own Nextcloud.
 * Off by default. Connecting opens Nextcloud's login page in the browser; Manent gets an app
 * password that stays in the OS keychain. The phone puts recordings in <folder>/Inbox.
 */
export function SyncSettings() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [server, setServer] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);

  useEffect(() => {
    call("sync_status").then(setStatus, (e: unknown) => setError(String(e)));
    const off = on("sync:changed", setStatus);
    return () => {
      off.then((f) => f());
    };
  }, []);

  const run = async (f: () => Promise<unknown>) => {
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    }
  };

  if (!status) return <p className="mt-1 text-sm text-muted-foreground">{error ?? "Loading…"}</p>;

  const connected = status.server !== null && status.state !== "connecting";

  return (
    <div className="mt-3 flex flex-col gap-4" data-testid="sync-settings" data-state={status.state}>
      {status.state === "connecting" ? (
        <div
          className="flex flex-col gap-2 rounded-lg border px-4 py-3 text-sm"
          data-testid="sync-connecting"
        >
          <p>Approve Manent in the browser window that just opened.</p>
          <p className="text-xs text-muted-foreground">
            Manent then gets its own app password, kept in this computer&apos;s keychain. You can
            revoke it here or in Nextcloud.
          </p>
          <div className="flex gap-2">
            {status.login_url && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => run(() => call("sync_open_login"))}
              >
                Open the login page again
              </Button>
            )}
            <Button
              size="sm"
              variant="ghost"
              onClick={() => run(() => call("sync_cancel_connect"))}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : !connected ? (
        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            run(() => call("sync_connect", { server }));
          }}
        >
          <label className="text-sm" htmlFor="sync-server">
            Nextcloud address
          </label>
          <div className="flex gap-2">
            <Input
              id="sync-server"
              value={server}
              onChange={(e) => setServer(e.target.value)}
              placeholder="cloud.example.org"
              autoComplete="url"
              data-testid="sync-server"
            />
            <Button size="sm" type="submit" disabled={!server.trim()} data-testid="sync-connect">
              Connect
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            Recordings from the phone app go into a Manent folder in your Nextcloud; this computer
            transcribes them and puts the transcript next to them. Voice data never leaves this
            computer.
          </p>
        </form>
      ) : (
        <>
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 text-sm" data-testid="sync-account">
              <p className="truncate">
                {status.login} · {status.server?.replace(/^https?:\/\//, "")}
              </p>
              <p className="text-xs text-muted-foreground" data-testid="sync-state">
                {status.state === "syncing"
                  ? "Syncing…"
                  : status.state === "error"
                    ? "Sync failed"
                    : status.last_sync
                      ? `Synced ${formatDateTime(status.last_sync)}`
                      : "Not synced yet"}
                {" · "}folder {status.folder}
              </p>
            </div>
            <Button
              size="sm"
              variant="outline"
              disabled={status.state === "syncing"}
              onClick={() => run(() => call("sync_now"))}
              data-testid="sync-now"
            >
              Sync now
            </Button>
          </div>
          {status.insecure && (
            <p className="text-xs text-destructive" role="note">
              This server uses http: recordings and transcripts travel unencrypted. Use https
              outside your own network.
            </p>
          )}
          {status.error && (
            <p className="text-xs text-destructive" role="alert" data-testid="sync-error">
              {status.error}
            </p>
          )}
          <fieldset className="flex flex-col gap-2">
            <legend className="mb-2 text-sm">What to sync</legend>
            {(
              [
                [
                  "phone",
                  "Recordings from the phone",
                  "Transcribed here; the transcript goes next to the recording.",
                ],
                [
                  "all",
                  "Also every meeting on this computer",
                  "Adds the playback file and transcript of each meeting.",
                ],
              ] as const
            ).map(([mode, label, hint]) => (
              <label key={mode} className="flex items-start gap-2 text-sm">
                <input
                  type="radio"
                  name="sync-mode"
                  className="mt-1 accent-brand"
                  checked={status.mode === mode}
                  onChange={() => run(() => call("sync_set_mode", { mode }))}
                  data-testid={`sync-mode-${mode}`}
                />
                <span>
                  {label}
                  <span className="block text-xs text-muted-foreground">{hint}</span>
                </span>
              </label>
            ))}
          </fieldset>
          <div className="flex items-center gap-2">
            {confirmDisconnect ? (
              <>
                <span className="text-xs text-muted-foreground">
                  Revoke Manent&apos;s access? Files stay in Nextcloud and here.
                </span>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() =>
                    run(async () => {
                      await call("sync_disconnect");
                      setConfirmDisconnect(false);
                    })
                  }
                  data-testid="sync-disconnect-confirm"
                >
                  Disconnect
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setConfirmDisconnect(false)}>
                  Cancel
                </Button>
              </>
            ) : (
              <Button
                size="sm"
                variant="ghost"
                className="text-muted-foreground hover:text-destructive"
                onClick={() => setConfirmDisconnect(true)}
                data-testid="sync-disconnect"
              >
                Disconnect
              </Button>
            )}
          </div>
        </>
      )}
      {error && (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
