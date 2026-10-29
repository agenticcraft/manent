import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export interface NavItem<K extends string> {
  key: K;
  label: string;
  icon: LucideIcon;
}

interface SidebarProps<K extends string> {
  items: NavItem<K>[];
  active: K | null;
  onSelect: (key: K) => void;
  /** Shown while a recording runs: a way back to the Live screen. */
  recording?: { active: boolean; onSelect: () => void };
}

/** Thin left sidebar (doc-3): app name, navigation, nothing else. */
export function Sidebar<K extends string>({ items, active, onSelect, recording }: SidebarProps<K>) {
  return (
    <nav
      aria-label="Main"
      className="flex w-52 shrink-0 flex-col border-r border-sidebar-border bg-sidebar px-3 py-5 text-sidebar-foreground"
    >
      <div className="mb-6 px-2 text-sm font-semibold tracking-tight text-foreground">Veritas</div>
      {recording && (
        <button
          type="button"
          onClick={recording.onSelect}
          aria-current={recording.active ? "page" : undefined}
          className={cn(
            "mb-3 flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-sm font-medium text-recording transition-colors",
            recording.active ? "bg-recording/10" : "hover:bg-recording/10",
          )}
          data-testid="recording-nav"
        >
          <span className="recording-pulse ml-1 size-2 rounded-full bg-recording" aria-hidden />
          Recording
        </button>
      )}
      <ul className="flex flex-col gap-0.5">
        {items.map(({ key, label, icon: Icon }) => (
          <li key={key}>
            <button
              type="button"
              aria-current={key === active ? "page" : undefined}
              onClick={() => onSelect(key)}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-sm transition-colors",
                key === active
                  ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground"
                  : "hover:bg-sidebar-accent/60",
              )}
            >
              <Icon className="size-4 opacity-70" aria-hidden />
              {label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
