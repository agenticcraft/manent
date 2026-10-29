import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

/** Calm empty state with an optional call to action. */
export function EmptyState({
  icon: Icon,
  title,
  children,
  action,
}: {
  icon: LucideIcon;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 py-24 text-center">
      <Icon className="size-8 text-muted-foreground" strokeWidth={1.5} aria-hidden />
      <h2 className="text-base font-medium">{title}</h2>
      {children && <p className="max-w-sm text-sm text-muted-foreground">{children}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
