import { Library as LibraryIcon, Settings as SettingsIcon, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { Sidebar, type NavItem } from "@/components/Sidebar";
import { followSystemTheme } from "@/lib/theme";
import { Library } from "@/routes/Library";
import { Settings } from "@/routes/Settings";
import { Speakers } from "@/routes/Speakers";

type Page = "library" | "speakers" | "settings";

const NAV: NavItem<Page>[] = [
  { key: "library", label: "Library", icon: LibraryIcon },
  { key: "speakers", label: "Speakers", icon: Users },
  { key: "settings", label: "Settings", icon: SettingsIcon },
];

const PAGES: Record<Page, () => React.JSX.Element> = {
  library: Library,
  speakers: Speakers,
  settings: Settings,
};

export default function App() {
  const [page, setPage] = useState<Page>("library");
  useEffect(followSystemTheme, []);
  const Current = PAGES[page];

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar items={NAV} active={page} onSelect={setPage} />
      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto px-10 py-8">
        <Current />
      </main>
    </div>
  );
}
