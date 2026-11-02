import { getCurrentWebview } from "@tauri-apps/api/webview";
import { FileAudio, Library as LibraryIcon, Settings as SettingsIcon, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { Sidebar, type NavItem } from "@/components/Sidebar";
import { call, on, type RecordingStarted } from "@/lib/ipc";
import { followSystemTheme } from "@/lib/theme";
import { Library } from "@/routes/Library";
import { Live } from "@/routes/Live";
import { Meeting } from "@/routes/Meeting";
import { Settings } from "@/routes/Settings";
import { Speakers } from "@/routes/Speakers";

type Page = "library" | "speakers" | "settings";

const NAV: NavItem<Page>[] = [
  { key: "library", label: "Library", icon: LibraryIcon },
  { key: "speakers", label: "Speakers", icon: Users },
  { key: "settings", label: "Settings", icon: SettingsIcon },
];

export default function App() {
  const [page, setPage] = useState<Page | "live">("library");
  const [session, setSession] = useState<RecordingStarted | null>(null);
  const [openMeeting, setOpenMeeting] = useState<number | null>(null);
  /** Where to open the meeting (a search match), ms. */
  const [focusMs, setFocusMs] = useState<number | undefined>(undefined);
  const showMeeting = (id: number, atMs?: number) => {
    setOpenMeeting(id);
    setFocusMs(atMs);
  };
  useEffect(followSystemTheme, []);
  const [dragging, setDragging] = useState(false);
  const [dropError, setDropError] = useState<string | null>(null);

  // Drop recordings on the window to import them (VRT-19); the Library shows the progress.
  useEffect(() => {
    const off = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "enter" || e.payload.type === "over") setDragging(true);
      else if (e.payload.type === "leave") setDragging(false);
      else if (e.payload.type === "drop") {
        setDragging(false);
        setDropError(null);
        call("import_audio", { paths: e.payload.paths }).then(
          () => {
            setPage((p) => (p === "live" ? p : "library"));
            setOpenMeeting(null);
          },
          (err: unknown) => setDropError(String(err)),
        );
      }
    });
    return () => {
      off.then((f) => f());
    };
  }, []);

  // Pick up a recording that is still running (e.g. after the UI reloaded).
  useEffect(() => {
    call("current_recording").then((s) => {
      if (s) {
        setSession(s);
        setPage("live");
      }
    });
  }, []);

  // The tray starts and stops recordings too: follow them (VRT-22).
  useEffect(() => {
    const offStarted = on("recording:started", (s) => {
      setSession(s);
      setPage("live");
    });
    const offStopped = on("recording:stopped", () => {
      setSession(null);
      setPage((p) => (p === "live" ? "library" : p));
    });
    return () => {
      offStarted.then((f) => f());
      offStopped.then((f) => f());
    };
  }, []);

  const startRecording = async () => {
    const s = await call("start_recording");
    setSession(s);
    setPage("live");
  };

  let content;
  if (page === "live" && session) {
    content = (
      <Live
        session={session}
        onStopped={() => {
          setSession(null);
          setPage("library");
        }}
      />
    );
  } else if (page === "library" && openMeeting !== null) {
    content = (
      <Meeting
        key={`${openMeeting}-${focusMs}`}
        id={openMeeting}
        focusMs={focusMs}
        onBack={() => setOpenMeeting(null)}
      />
    );
  } else if (page === "speakers") {
    content = <Speakers />;
  } else if (page === "settings") {
    content = <Settings />;
  } else {
    content = (
      <Library onNewRecording={startRecording} onOpenMeeting={showMeeting} recording={!!session} />
    );
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar
        items={NAV}
        active={page === "live" ? null : page}
        onSelect={(p) => {
          setPage(p);
          setOpenMeeting(null);
        }}
        recording={
          session ? { active: page === "live", onSelect: () => setPage("live") } : undefined
        }
      />
      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto px-10 py-8">
        <div className="mx-auto flex w-full max-w-3xl min-w-0 flex-1 flex-col">
          {dropError && (
            <p role="alert" className="mb-4 text-sm text-destructive" data-testid="drop-error">
              {dropError}
            </p>
          )}
          {content}
        </div>
      </main>
      {dragging && (
        <div
          className="pointer-events-none fixed inset-3 z-50 flex items-center justify-center rounded-xl border-2 border-dashed border-brand bg-background/80"
          data-testid="drop-overlay"
        >
          <p className="flex items-center gap-2 text-sm font-medium">
            <FileAudio className="size-4" /> Drop recordings to import them
          </p>
        </div>
      )}
    </div>
  );
}
