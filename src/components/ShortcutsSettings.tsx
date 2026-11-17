import { useEffect, useState } from "react";
import { ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  call,
  on,
  type PasteKey,
  type PasteStatus,
  type ShortcutStatus,
  type ShortcutView,
} from "@/lib/ipc";

/** The keys asked for; the desktop shows what it bound once they are on. */
const RECORDING: readonly ShortcutView[] = [
  { id: "toggle-recording", description: "Start or stop recording", trigger: "Ctrl+Alt+R" },
  { id: "mark-moment", description: "Mark this moment", trigger: "Ctrl+Alt+M" },
];
const DICTATE: ShortcutView = {
  id: "dictate",
  description: "Hold to dictate",
  trigger: "Ctrl+Shift+Space",
};

type Group = "recording" | "dictation";

function KeyRow({ shortcut, bound }: { shortcut: ShortcutView; bound: boolean }) {
  return (
    <li
      className="flex items-center justify-between gap-4"
      data-testid="shortcut-row"
      data-id={shortcut.id}
    >
      <span>{shortcut.description}</span>
      <kbd
        className={
          "rounded border bg-muted px-1.5 py-0.5 font-sans text-xs tabular-nums " +
          (bound ? "text-foreground" : "text-muted-foreground")
        }
      >
        {shortcut.trigger ?? "No keys"}
      </kbd>
    </li>
  );
}

/** Typing the dictated text (VRT-59), off until turned on; the desktop asks once. */
function AutoPasteSettings() {
  const [status, setStatus] = useState<PasteStatus | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    call("get_dictation_paste").then(setStatus, () => {});
    const off = on("dictation:paste", setStatus);
    return () => {
      off.then((f) => f());
    };
  }, []);

  if (!status?.supported) return null;

  const turn = async (enabled: boolean) => {
    setBusy(true);
    try {
      setStatus(await call("set_dictation_paste", { enabled }));
    } catch (e) {
      setStatus({ ...status, error: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3" data-testid="autopaste-settings">
      <div className="flex items-center justify-between gap-4 text-sm">
        <span>
          Type automatically
          <span className="block text-xs text-muted-foreground">
            Types the text where the cursor is. Characters your keyboard can’t type are pasted, and
            your clipboard is put back.
          </span>
        </span>
        {status.enabled ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => turn(false)}
            disabled={busy}
            data-testid="autopaste-off"
          >
            Turn off
          </Button>
        ) : (
          <Button
            variant="outline"
            size="sm"
            onClick={() => turn(true)}
            disabled={busy}
            data-testid="autopaste-on"
          >
            {busy ? "Waiting for the desktop…" : "Turn on"}
          </Button>
        )}
      </div>
      {busy && !status.enabled && (
        <p className="text-xs text-muted-foreground" role="status">
          Your desktop asks once. Switch on “Allow Remote Interaction”, then choose Share.
        </p>
      )}
      {status.enabled && (
        <label className="flex items-center justify-between gap-4 text-sm">
          <span>
            Paste with
            <span className="block text-xs text-muted-foreground">
              For characters your keyboard can’t type. Terminals paste with Ctrl+Shift+V.
            </span>
          </span>
          <span className="relative flex items-center">
            <select
              value={status.key}
              onChange={(e) => {
                const key = e.target.value as PasteKey;
                setStatus({ ...status, key });
                call("set_paste_key", { key }).then(setStatus, () => {});
              }}
              className="h-8 appearance-none rounded-md border bg-background pr-8 pl-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              data-testid="paste-key"
            >
              <option value="ctrl-v">Ctrl+V</option>
              <option value="ctrl-shift-v">Ctrl+Shift+V</option>
            </select>
            <ChevronDown className="pointer-events-none absolute right-2 size-4 text-muted-foreground" />
          </span>
        </label>
      )}
      {status.error && (
        <p className="text-sm text-muted-foreground" role="status" data-testid="autopaste-message">
          {status.error}
        </p>
      )}
    </div>
  );
}

/**
 * Settings > Shortcuts: global keys through the desktop (VRT-40) and hold-to-dictate (VRT-58),
 * each off until turned on.
 */
export function ShortcutsSettings() {
  const [status, setStatus] = useState<ShortcutStatus | null>(null);
  const [busy, setBusy] = useState<Group | null>(null);
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

  const turn = async (group: Group, enabled: boolean) => {
    setMessage(null);
    setBusy(group);
    try {
      const s = await call(
        group === "recording" ? "set_shortcuts_enabled" : "set_dictation_enabled",
        { enabled },
      );
      setStatus(s);
      if (s.error) setMessage(s.error);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(null);
    }
  };

  const bound = (s: ShortcutView) => status.shortcuts.find((b) => b.id === s.id) ?? s;
  const toggle = (group: Group, enabled: boolean, testid: string) =>
    enabled ? (
      <Button
        variant="ghost"
        size="sm"
        onClick={() => turn(group, false)}
        disabled={busy !== null}
        data-testid={`${testid}-off`}
      >
        Turn off
      </Button>
    ) : (
      <Button
        variant="outline"
        size="sm"
        onClick={() => turn(group, true)}
        disabled={busy !== null}
        data-testid={`${testid}-on`}
      >
        {busy === group ? "Waiting for the desktop…" : "Turn on"}
      </Button>
    );

  return (
    <div className="mt-3 flex flex-col gap-5" data-testid="shortcuts-settings">
      <div className="flex flex-col gap-3">
        <ul className="flex flex-col gap-2 text-sm">
          {RECORDING.map((s) => (
            <KeyRow key={s.id} shortcut={status.enabled ? bound(s) : s} bound={status.enabled} />
          ))}
        </ul>
        <div className="flex flex-wrap items-center gap-2">
          {toggle("recording", status.enabled, "shortcuts")}
        </div>
      </div>

      <div className="flex flex-col gap-3 border-t pt-4" data-testid="dictation-settings">
        <div>
          <p className="text-sm font-medium">Dictation</p>
          <p className="mt-0.5 text-sm text-muted-foreground">
            Hold the key and speak. When you let go, the text is on the clipboard, ready to paste.
            Only the microphone is used, and nothing is saved.
          </p>
        </div>
        <ul className="flex flex-col gap-2 text-sm">
          <KeyRow shortcut={status.dictation ? bound(DICTATE) : DICTATE} bound={status.dictation} />
        </ul>
        <div className="flex flex-wrap items-center gap-2">
          {toggle("dictation", status.dictation, "dictation")}
        </div>
        {status.dictation && <AutoPasteSettings />}
      </div>

      {(status.enabled || status.dictation) && (
        <div>
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
        </div>
      )}
      {busy && (
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
