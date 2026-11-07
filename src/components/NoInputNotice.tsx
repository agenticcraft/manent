import { CircleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { call, type SourceKind } from "@/lib/ipc";

/** Whether the OS asks consent to capture audio, so the notice can open its settings (macOS). */
function usePrivacySettings(): boolean {
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    call("app_info").then(
      (info) => setAvailable(info.privacy_settings),
      () => {},
    );
  }, []);
  return available;
}

/**
 * A source opened but nothing arrives from it (VRT-12): on macOS that is what capture does
 * without the user's consent, and a dead device looks the same. `then` says what to do once
 * it is allowed (start a new recording, or come back to the meters).
 */
export function NoInputNotice({ kind, then }: { kind: SourceKind; then: string }) {
  const privacySettings = usePrivacySettings();
  const isMic = kind === "mic";
  const title = isMic
    ? "No sound is coming in from the microphone."
    : "No system audio is coming in.";
  const hint = privacySettings
    ? `Manent may not have permission to record ${isMic ? "the microphone" : "the system audio"}. Allow it in System Settings, then ${then}.`
    : isMic
      ? "Check that the microphone is connected and switched on."
      : "Check that this output device is in use and working.";
  return (
    <div
      role="alert"
      className="flex items-start gap-3 rounded-lg border px-4 py-3 text-sm"
      data-testid="no-input-notice"
      data-kind={kind}
    >
      <CircleAlert className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="font-medium">{title}</p>
        <p className="text-muted-foreground">{hint}</p>
      </div>
      {privacySettings && (
        <Button
          variant="outline"
          size="sm"
          onClick={() => call("open_privacy_settings", { kind }).catch(() => {})}
          data-testid="open-privacy-settings"
        >
          Open System Settings
        </Button>
      )}
    </div>
  );
}
