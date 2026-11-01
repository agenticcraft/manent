import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/format";
import { ChevronDown } from "lucide-react";
import { call, on, type AppSettings, type EngineStatus } from "@/lib/ipc";

const ITEM_LABEL = { engine: "Engine", model: "Model", notice: "Licences" } as const;

const HELPER_LABEL = {
  stopped: "Ready to start",
  starting: "Starting…",
  ready: "Running",
  restarting: "Restarting after a crash…",
  failed: "Not running",
} as const;

/** Settings > Transcription: download the engine and model, start the engine, licences. */
export function TranscriptionSettings() {
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [showNotice, setShowNotice] = useState(false);

  useEffect(() => {
    call("engine_status").then(setStatus, (e: unknown) => setError(String(e)));
    const unlisten = on("engine:status", setStatus);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const installed = !!status?.engine_installed && !!status?.model_installed;
  useEffect(() => {
    if (installed) call("engine_notice").then(setNotice, () => {});
  }, [installed]);

  const run = async (action: () => Promise<unknown>) => {
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    }
  };

  if (!status) {
    return <p className="mt-1 text-sm text-muted-foreground">{error ?? "Loading…"}</p>;
  }
  if (!status.supported) {
    return (
      <p className="mt-3 text-sm text-muted-foreground" data-testid="engine-unsupported">
        There is no prebuilt transcription engine for this platform yet.
      </p>
    );
  }

  const dl = status.download;
  const fraction = dl && dl.total > 0 ? dl.received / dl.total : 0;
  const shownError = error ?? status.download_error ?? status.helper_error;

  return (
    <div className="mt-3 flex flex-col gap-3" data-testid="engine-settings">
      {!installed && !dl && (
        <div className="flex items-center justify-between gap-4 rounded-lg border px-4 py-3">
          <div className="text-sm">
            <p>Transcription runs on this computer.</p>
            <p className="text-muted-foreground tabular-nums">
              One-time download: engine and model, {formatBytes(status.download_size)}.
            </p>
          </div>
          <Button onClick={() => run(() => call("engine_download"))} data-testid="engine-download">
            Download
          </Button>
        </div>
      )}

      {dl && (
        <div
          className="flex flex-col gap-2 rounded-lg border px-4 py-3"
          data-testid="engine-progress"
        >
          <div className="flex items-center justify-between gap-4 text-sm">
            <span>
              Downloading {ITEM_LABEL[dl.item].toLowerCase()}
              <span className="ml-2 text-muted-foreground tabular-nums">
                {dl.total > 0
                  ? `${formatBytes(dl.received)} of ${formatBytes(dl.total)}`
                  : "Preparing…"}
              </span>
            </span>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => run(() => call("engine_cancel_download"))}
              data-testid="engine-cancel"
            >
              Cancel
            </Button>
          </div>
          <div
            className="h-1.5 overflow-hidden rounded-full bg-muted"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(fraction * 100)}
          >
            <div
              className="h-full rounded-full bg-brand transition-[width] duration-300"
              style={{ width: `${fraction * 100}%` }}
            />
          </div>
        </div>
      )}

      {installed && (
        <div className="flex items-center justify-between gap-4 rounded-lg border px-4 py-3">
          <div className="text-sm">
            <p>
              Parakeet 0.6B v3 <span className="text-muted-foreground">· Dutch and English</span>
            </p>
            <p className="text-muted-foreground tabular-nums" data-testid="engine-state">
              {HELPER_LABEL[status.helper]}
              {status.helper === "ready" &&
                status.helper_info &&
                ` · loaded in ${(status.helper_info.load_ms / 1000).toFixed(1)} s`}
              {status.restarts > 0 && ` · restarted ${status.restarts}×`}
            </p>
          </div>
          {status.helper !== "ready" && (
            <Button
              variant="outline"
              disabled={status.helper === "starting" || status.helper === "restarting"}
              onClick={() => run(() => call("engine_start").then(setStatus))}
              data-testid="engine-start"
            >
              Start engine
            </Button>
          )}
        </div>
      )}

      <EngineOptions installed={installed || status.engine_installed} busy={!!dl} />

      {shownError && (
        <p role="alert" className="text-sm text-destructive" data-testid="engine-error">
          {shownError}
        </p>
      )}

      {notice && (
        <div>
          <Button
            variant="link"
            size="sm"
            className="h-auto px-0 text-muted-foreground"
            onClick={() => setShowNotice((v) => !v)}
            aria-expanded={showNotice}
            data-testid="engine-licences"
          >
            {showNotice ? "Hide model licences" : "Model licences and attributions"}
          </Button>
          {showNotice && (
            <pre
              className="mt-2 max-h-72 overflow-auto rounded-lg bg-muted px-4 py-3 font-sans text-xs leading-relaxed whitespace-pre-wrap text-muted-foreground"
              data-testid="engine-notice"
            >
              {notice}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

/** The update interval while recording, and engine maintenance (VRT-27). */
function EngineOptions({ installed, busy }: { installed: boolean; busy: boolean }) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  useEffect(() => {
    call("get_app_settings").then(setSettings, () => {});
  }, []);

  const act = async (action: () => Promise<string>) => {
    setResult(null);
    try {
      setResult(await action());
    } catch (e) {
      setResult(String(e));
    }
  };

  const check = () =>
    act(async () => {
      setChecking(true);
      try {
        const damaged = await call("engine_check");
        return damaged.length === 0
          ? "Engine and model are intact."
          : `Damaged and removed: ${damaged.join(", ")}. Download them again above.`;
      } finally {
        setChecking(false);
      }
    });

  return (
    <div className="flex flex-col gap-3" data-testid="engine-options">
      {settings && (
        <label className="flex items-center justify-between gap-4 text-sm">
          <span>
            Update speakers while recording
            <span className="block text-xs text-muted-foreground">
              More often costs more CPU; less often names speakers later. Applies to the next
              recording.
            </span>
          </span>
          <span className="relative flex items-center">
            <select
              value={settings.rolling_interval_s}
              onChange={(e) => {
                const seconds = Number(e.target.value);
                setSettings({ ...settings, rolling_interval_s: seconds });
                call("set_rolling_interval", { seconds }).catch((err: unknown) =>
                  setResult(String(err)),
                );
              }}
              className="h-8 appearance-none rounded-md border bg-background pr-8 pl-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              data-testid="rolling-interval"
            >
              {settings.rolling_intervals.map((s) => (
                <option key={s} value={s}>
                  every {s} s
                </option>
              ))}
            </select>
            <ChevronDown className="pointer-events-none absolute right-2 size-4 text-muted-foreground" />
          </span>
        </label>
      )}
      {installed && (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={check}
            disabled={checking || busy}
            data-testid="engine-check"
          >
            {checking ? "Checking…" : "Check model"}
          </Button>
          {confirmRemove ? (
            <>
              <span className="text-xs text-muted-foreground">
                Remove the engine and model (1.1 GB)? Transcription stops until you download them
                again.
              </span>
              <Button
                variant="destructive"
                size="sm"
                onClick={() => {
                  setConfirmRemove(false);
                  act(async () => {
                    await call("engine_remove");
                    return "Engine and model removed.";
                  });
                }}
                data-testid="engine-remove-confirm"
              >
                Remove
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setConfirmRemove(false)}>
                Cancel
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setConfirmRemove(true)}
              disabled={busy}
              data-testid="engine-remove"
            >
              Remove engine and model
            </Button>
          )}
        </div>
      )}
      {result && (
        <p className="text-sm text-muted-foreground" role="status" data-testid="engine-result">
          {result}
        </p>
      )}
    </div>
  );
}
