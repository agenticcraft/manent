import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { call, type AppSettings } from "@/lib/ipc";

/** Settings > Storage (VRT-27): where meetings are kept and which audio stays. */
export function StorageSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [moving, setMoving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    call("get_app_settings").then(setSettings, (e: unknown) => setMessage(String(e)));
  }, []);

  if (!settings)
    return <p className="mt-1 text-sm text-muted-foreground">{message ?? "Loading…"}</p>;

  const move = async (to: string | null) => {
    setMessage(null);
    setMoving(true);
    try {
      setSettings(await call("move_meetings", { to }));
      setMessage("Meetings moved.");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setMoving(false);
    }
  };

  const choose = async () => {
    const picked = await open({ directory: true, title: "Folder for meetings" });
    if (typeof picked === "string") await move(picked);
  };

  const custom = settings.meetings_dir !== settings.default_meetings_dir;

  return (
    <div className="mt-3 flex flex-col gap-4" data-testid="storage-settings">
      <div className="flex items-center justify-between gap-4 text-sm">
        <span className="min-w-0">
          Meetings folder
          <span
            className="block truncate text-xs text-muted-foreground"
            title={settings.meetings_dir}
            data-testid="meetings-dir"
          >
            {settings.meetings_dir}
          </span>
        </span>
        <span className="flex shrink-0 gap-2">
          {custom && (
            <Button variant="ghost" size="sm" onClick={() => move(null)} disabled={moving}>
              Back to default
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={choose}
            disabled={moving}
            data-testid="choose-meetings-dir"
          >
            {moving ? "Moving…" : "Change…"}
          </Button>
        </span>
      </div>
      <fieldset className="flex flex-col gap-2 text-sm">
        <legend className="mb-1">Audio to keep after transcription</legend>
        {(
          [
            [
              "all",
              "Tracks and playback file",
              "Lets you re-transcribe later. A recording takes about 400 MB per hour.",
            ],
            [
              "mix",
              "Playback file only",
              "About 13 MB per hour; re-transcribing is no longer possible.",
            ],
          ] as const
        ).map(([value, label, hint]) => (
          <label key={value} className="flex items-start gap-2">
            <input
              type="radio"
              name="keep-audio"
              value={value}
              checked={settings.keep_audio === value}
              onChange={() => {
                setSettings({ ...settings, keep_audio: value });
                call("set_keep_audio", { keep: value }).catch((e: unknown) =>
                  setMessage(String(e)),
                );
              }}
              className="mt-1 accent-brand"
              data-testid={`keep-${value}`}
            />
            <span>
              {label}
              <span className="block text-xs text-muted-foreground">{hint}</span>
            </span>
          </label>
        ))}
      </fieldset>
      {message && (
        <p className="text-sm text-muted-foreground" role="status" data-testid="storage-message">
          {message}
        </p>
      )}
    </div>
  );
}
