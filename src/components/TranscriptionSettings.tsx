import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { formatBytes } from "@/lib/format";
import { call, on, type EngineStatus } from "@/lib/ipc";

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
