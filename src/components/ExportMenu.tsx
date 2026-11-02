import { save } from "@tauri-apps/plugin-dialog";
import { Download } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { useDismiss } from "@/lib/dismiss";
import { Button } from "@/components/ui/button";
import { call, type ExportFormat } from "@/lib/ipc";

const FORMATS: { format: ExportFormat; label: string; name: string }[] = [
  { format: "md", label: "Markdown", name: "Markdown" },
  { format: "txt", label: "Plain text", name: "Text" },
  { format: "srt", label: "Subtitles (SRT)", name: "SubRip subtitles" },
  { format: "json", label: "JSON with word times", name: "JSON" },
];

/** Export the transcript (VRT-21): pick a format, then where to save it. */
export function ExportMenu({
  meetingId,
  title,
  dateLabel,
  disabled,
  onError,
}: {
  meetingId: number;
  title: string;
  dateLabel: string;
  disabled?: boolean;
  onError: (e: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const close = useCallback(() => setOpen(false), []);
  useDismiss(ref, open, close);

  const exportAs = async (f: (typeof FORMATS)[number]) => {
    setOpen(false);
    try {
      // File names: the title without characters that file systems refuse.
      const base = title.replace(/[\\/:*?"<>|]+/g, " ").trim();
      const path = await save({
        title: "Export transcript",
        defaultPath: `${base}.${f.format}`,
        filters: [{ name: f.name, extensions: [f.format] }],
      });
      if (path) await call("export_meeting", { id: meetingId, format: f.format, path, dateLabel });
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <div ref={ref} className="relative">
      <Button
        variant="outline"
        size="sm"
        onClick={() => setOpen((o) => !o)}
        disabled={disabled}
        aria-expanded={open}
        data-testid="export"
      >
        <Download /> Export
      </Button>
      {open && (
        <ul
          role="menu"
          className="absolute top-full right-0 z-20 mt-1 w-56 rounded-lg border bg-popover p-1 text-sm text-popover-foreground shadow-lg"
          data-testid="export-menu"
        >
          {FORMATS.map((f) => (
            <li key={f.format}>
              <button
                type="button"
                role="menuitem"
                className="w-full rounded-md px-2 py-1.5 text-left hover:bg-muted"
                onClick={() => exportAs(f)}
                data-format={f.format}
              >
                {f.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
