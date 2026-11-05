import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { call, on, type ShortcutStatus } from "@/lib/ipc";

/** The keys asked for; the desktop shows what it bound once the shortcuts are on. */
const WANTED = [
  { id: "toggle-recording", description: "Start or stop recording", trigger: "Ctrl+Alt+R" },
  { id: "mark-moment", description: "Mark this moment", trigger: "Ctrl+Alt+M" },
] as const;

/** Settings > Shortcuts (VRT-40): global keys through the desktop, off until turned on. */
export function ShortcutsSettings() {
  const [status, setStatus] = useState<ShortcutStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    call("get_shortcuts").then(setStatus, (e: unknown) => setMessage(String(e)));
    const off = on("shortcuts:changed", setStatus);
    return () => {
      off.then((f) => f());
    };
  }, []);

  if (!status) return <p className="mt-1 text-sm text-muted-foreground">{message ?? "Loading…"}</p>;
  if (!status.supported)
    return (
      <p className="mt-3 text-sm text-muted-foreground" data-testid="shortcuts-unsupported">
        Not available on this system yet.
      </p>
    );

  const turn = async (enabled: boolean) => {
    setMessage(null);
    setBusy(true);
    try {
      const s = await call("set_shortcuts_enabled", { enabled });
      setStatus(s);
      if (s.error) setMessage(s.error);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  };

  const rows = status.enabled ? status.shortcuts : WANTED;
  return (
    <div className="mt-3 flex flex-col gap-3" data-testid="shortcuts-settings">
      <ul className="flex flex-col gap-2 text-sm">
        {rows.map((s) => (
          <li
            key={s.id}
            className="flex items-center justify-between gap-4"
            data-testid="shortcut-row"
            data-id={s.id}
          >
            <span>{s.description}</span>
            <kbd
              className={
                "rounded border bg-muted px-1.5 py-0.5 font-sans text-xs tabular-nums " +
                (status.enabled ? "text-foreground" : "text-muted-foreground")
              }
            >
              {s.trigger ?? "No keys"}
            </kbd>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap items-center gap-2">
        {status.enabled ? (
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                call("open_shortcut_settings").catch((e: unknown) => setMessage(String(e)))
              }
              data-testid="change-shortcuts"
            >
              Change keys…
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => turn(false)}
              disabled={busy}
              data-testid="shortcuts-off"
            >
              Turn off
            </Button>
          </>
        ) : (
          <Button
            variant="outline"
            size="sm"
            onClick={() => turn(true)}
            disabled={busy}
            data-testid="shortcuts-on"
          >
            {busy ? "Waiting for the desktop…" : "Turn on"}
          </Button>
        )}
      </div>
      {busy && !status.enabled && (
        <p className="text-xs text-muted-foreground" role="status">
          Your desktop asks once whether Manent may use these keys.
        </p>
      )}
      {message && (
        <p className="text-sm text-muted-foreground" role="status" data-testid="shortcuts-message">
          {message}
        </p>
      )}
    </div>
  );
}
