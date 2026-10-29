import { ChevronDown } from "lucide-react";
import { useEffect, useState } from "react";
import { LevelMeter } from "@/components/LevelMeter";
import { call, on, type SourceInfo, type SourceSettings } from "@/lib/ipc";

const NO_SYSTEM = "";

/** Settings > Sources: which microphone and system audio to record, with live test meters. */
export function SourcesSettings() {
  const [settings, setSettings] = useState<SourceSettings | null>(null);
  const [levels, setLevels] = useState<Record<string, number>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("get_recording_sources").then(setSettings, (e: unknown) => setError(String(e)));
  }, []);

  // Test meters: monitor the selected devices while this screen is open.
  const selected = settings ? [settings.mic, settings.system].filter((x): x is string => !!x) : [];
  const selectedKey = selected.join("|");
  useEffect(() => {
    if (!selectedKey) return;
    const ids = selectedKey.split("|");
    call("start_source_monitor", { sourceIds: ids }).catch((e: unknown) => setError(String(e)));
    const unlisten = on("audio:monitor-level", (e) =>
      setLevels((l) => (l[e.source_id] === e.rms_db ? l : { ...l, [e.source_id]: e.rms_db })),
    );
    return () => {
      unlisten.then((f) => f());
      call("stop_source_monitor").catch(() => {});
    };
  }, [selectedKey]);

  const save = async (next: SourceSettings) => {
    setSettings(next);
    setError(null);
    if (!next.mic) return;
    try {
      await call("set_recording_sources", { mic: next.mic, system: next.system });
    } catch (e) {
      setError(String(e));
    }
  };

  if (!settings) {
    return <p className="mt-1 text-sm text-muted-foreground">{error ?? "Loading…"}</p>;
  }
  const mics = settings.available.filter((s) => s.kind === "mic");
  const outputs = settings.available.filter((s) => s.kind === "loopback");

  return (
    <div className="mt-3 flex flex-col gap-3">
      <SourceRow
        label="Microphone"
        testId="mic-select"
        value={settings.mic ?? ""}
        options={mics}
        rmsDb={settings.mic ? levels[settings.mic] : undefined}
        onChange={(id) => save({ ...settings, mic: id })}
      />
      <SourceRow
        label="System audio"
        testId="system-select"
        value={settings.system ?? NO_SYSTEM}
        options={outputs}
        allowNone
        rmsDb={settings.system ? levels[settings.system] : undefined}
        onChange={(id) => save({ ...settings, system: id === NO_SYSTEM ? null : id })}
      />
      <p className="text-xs text-muted-foreground">
        “System default” follows the device chosen in your operating system, also when it changes
        during a meeting.
      </p>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

function SourceRow({
  label,
  testId,
  value,
  options,
  allowNone,
  rmsDb,
  onChange,
}: {
  label: string;
  testId: string;
  value: string;
  options: SourceInfo[];
  allowNone?: boolean;
  rmsDb?: number;
  onChange: (id: string) => void;
}) {
  return (
    <label className="flex items-center gap-4 text-sm">
      <span className="w-28 shrink-0 text-muted-foreground">{label}</span>
      <span className="relative min-w-0 flex-1">
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="h-8 w-full appearance-none truncate rounded-md border bg-background pr-8 pl-2.5 text-sm text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          data-testid={testId}
        >
          {options.map((o) => (
            <option key={o.id} value={o.id}>
              {o.name}
            </option>
          ))}
          {allowNone && <option value={NO_SYSTEM}>Don’t record system audio</option>}
        </select>
        <ChevronDown
          className="pointer-events-none absolute top-1/2 right-2.5 size-4 -translate-y-1/2 text-muted-foreground"
          aria-hidden
        />
      </span>
      <LevelMeter rmsDb={value ? rmsDb : undefined} />
    </label>
  );
}
