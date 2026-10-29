import { useEffect, useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { call, type AppInfo } from "@/lib/ipc";

const SECTIONS = [
  { title: "Sources", text: "Choose the microphone and the output device to record." },
  {
    title: "Transcription",
    text: "Model status and download. Dutch and English are detected automatically.",
  },
  { title: "Storage", text: "Where recordings are kept and in which format." },
];

/** Settings (doc-3 screen 5). Sections are filled in by VRT-9 and VRT-27. */
export function Settings() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("app_info").then(setInfo, (e: unknown) => setError(String(e)));
  }, []);

  return (
    <>
      <PageHeader title="Settings" />
      <div className="flex max-w-2xl flex-col divide-y divide-border">
        {SECTIONS.map((s) => (
          <section key={s.title} className="py-5 first:pt-0">
            <h2 className="text-sm font-medium">{s.title}</h2>
            <p className="mt-1 text-sm text-muted-foreground">{s.text}</p>
          </section>
        ))}
        <section className="py-5" aria-label="About">
          <h2 className="text-sm font-medium">About</h2>
          <p className="mt-1 text-sm text-muted-foreground tabular-nums" data-testid="app-info">
            {info
              ? `${info.name} ${info.version} · ${info.os}/${info.arch}`
              : (error ?? "Loading…")}
          </p>
        </section>
      </div>
    </>
  );
}
