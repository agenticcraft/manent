import { Library as LibraryIcon, Settings as SettingsIcon, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { Sidebar, type NavItem } from "@/components/Sidebar";
import { call, type RecordingStarted } from "@/lib/ipc";
import { followSystemTheme } from "@/lib/theme";
import { Library } from "@/routes/Library";
import { Live } from "@/routes/Live";
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
  useEffect(followSystemTheme, []);

  // Pick up a recording that is still running (e.g. after the UI reloaded).
  useEffect(() => {
    call("current_recording").then((s) => {
      if (s) {
        setSession(s);
        setPage("live");
      }
    });
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
  } else if (page === "speakers") {
    content = <Speakers />;
  } else if (page === "settings") {
    content = <Settings />;
  } else {
    content = <Library onNewRecording={startRecording} recording={!!session} />;
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar
        items={NAV}
        active={page === "live" ? null : page}
        onSelect={setPage}
        recording={
          session ? { active: page === "live", onSelect: () => setPage("live") } : undefined
        }
      />
      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto px-10 py-8">{content}</main>
    </div>
  );
}
