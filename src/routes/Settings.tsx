import { useEffect, useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { SourcesSettings } from "@/components/SourcesSettings";
import { StorageSettings } from "@/components/StorageSettings";
import { TranscriptionSettings } from "@/components/TranscriptionSettings";
import { call, type AppInfo } from "@/lib/ipc";

const SECTIONS = [
  {
    title: "Sources",
    text: "Choose the microphone and the system audio to record.",
    body: <SourcesSettings />,
  },
  {
    title: "Transcription",
    text: "The engine and model run locally. Dutch and English are detected automatically.",
    body: <TranscriptionSettings />,
  },
  {
    title: "Storage",
    text: "Where meetings are kept and which audio stays after transcription.",
    body: <StorageSettings />,
  },
];

/** Settings (doc-3 screen 5). */
export function Settings() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("app_info").then(setInfo, (e: unknown) => setError(String(e)));
  }, []);

  return (
    <>
      <PageHeader title="Settings" />
      <div className="flex flex-col divide-y divide-border">
        {SECTIONS.map((s) => (
          <section key={s.title} className="py-5 first:pt-0">
            <h2 className="text-sm font-medium">{s.title}</h2>
            <p className="mt-1 text-sm text-muted-foreground">{s.text}</p>
            {"body" in s && s.body}
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
