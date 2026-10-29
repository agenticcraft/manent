import { FileAudio, Mic, NotebookText } from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";

/** Library (doc-3 screen 1). Meetings list comes with VRT-7; recording with VRT-8. */
export function Library() {
  return (
    <>
      <PageHeader
        title="Library"
        actions={
          <>
            <Button variant="ghost" size="sm" disabled>
              <FileAudio /> Import audio
            </Button>
            <Button size="sm" className="bg-brand text-brand-foreground hover:bg-brand/90" disabled>
              <Mic /> New recording
            </Button>
          </>
        }
      />
      <EmptyState icon={NotebookText} title="No meetings yet">
        Record a meeting or import an audio file. Transcripts in Dutch and English appear here.
      </EmptyState>
    </>
  );
}
