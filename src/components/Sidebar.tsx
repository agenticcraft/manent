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
  /** Always on top (VRT-49): goes to the record page, or back to the running recording. */
  record: { recording: boolean; active: boolean; onSelect: () => void };
}

/** Thin left sidebar (doc-3): app name, navigation, nothing else. */
export function Sidebar<K extends string>({ items, active, onSelect, record }: SidebarProps<K>) {
  return (
    <nav
      aria-label="Main"
      className="flex w-52 shrink-0 flex-col border-r border-sidebar-border bg-sidebar px-3 py-5 text-sidebar-foreground"
    >
      <div className="mb-6 px-2 text-sm font-semibold tracking-tight text-foreground">Manent</div>
      <button
        type="button"
        onClick={record.onSelect}
        aria-current={record.active ? "page" : undefined}
        className={cn(
          "mb-3 flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-sm font-medium transition-colors",
          record.recording
            ? cn("text-recording", record.active ? "bg-recording/10" : "hover:bg-recording/10")
            : record.active
              ? "bg-sidebar-accent text-sidebar-accent-foreground"
              : "hover:bg-sidebar-accent/60",
        )}
        data-testid={record.recording ? "recording-nav" : "record-nav"}
      >
        <span
          className={cn(
            "ml-1 size-2 rounded-full bg-recording",
            record.recording && "recording-pulse",
          )}
          aria-hidden
        />
        {record.recording ? "Recording" : "Record"}
      </button>
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
