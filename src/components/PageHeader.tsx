import type { ReactNode } from "react";

/** Quiet page header: title on the left, actions on the right, no heavy borders. */
export function PageHeader({ title, actions }: { title: string; actions?: ReactNode }) {
  return (
    <header className="flex items-center justify-between gap-4 pb-6">
      <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  );
}
