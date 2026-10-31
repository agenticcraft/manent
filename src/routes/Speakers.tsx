import { ChevronDown, Trash2, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import { initials, speakerBg } from "@/lib/format";
import { call, on, type Person } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** Speakers (doc-3 screen 4): known voices with Rename, Merge and Delete (VRT-18). */
export function Speakers() {
  const [people, setPeople] = useState<Person[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const load = () => call("list_people").then(setPeople, (e: unknown) => setError(String(e)));
    load();
    const off = on("speakers:changed", load);
    return () => {
      off.then((f) => f());
    };
  }, []);

  const run = async (action: () => Promise<unknown>) => {
    setError(null);
    try {
      await action();
      setPeople(await call("list_people"));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <>
      <PageHeader title="Speakers" />
      {error && (
        <p role="alert" className="pb-4 text-sm text-destructive">
          {error}
        </p>
      )}
      {people && people.length === 0 && (
        <EmptyState icon={Users} title="No known voices yet">
          Name a speaker in a transcript and Veritas recognizes them in later meetings. Voice data
          stays on this computer.
        </EmptyState>
      )}
      {people && people.length > 0 && (
        <div className="flex max-w-2xl flex-col divide-y divide-border" data-testid="people">
          {people.map((p) => (
            <PersonRow
              key={`${p.id}-${p.name}`}
              person={p}
              others={people.filter((o) => o.id !== p.id)}
              run={run}
            />
          ))}
          <p className="pt-4 text-xs text-muted-foreground">
            Voice samples are biometric data and never leave this computer. Deleting a person
            deletes their voice samples; their words stay in the transcripts, unnamed.
          </p>
        </div>
      )}
    </>
  );
}

function PersonRow({
  person,
  others,
  run,
}: {
  person: Person;
  others: Person[];
  run: (action: () => Promise<unknown>) => Promise<void>;
}) {
  const [name, setName] = useState(person.name);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const rename = () => {
    if (name.trim() && name.trim() !== person.name) {
      run(() => call("rename_person", { id: person.id, name }));
    } else {
      setName(person.name);
    }
  };

  return (
    <div className="flex items-center gap-3 py-3" data-testid="person" data-person-id={person.id}>
      <span
        className={cn(
          "flex size-8 shrink-0 items-center justify-center rounded-full text-xs font-medium text-foreground",
          speakerBg(person.color),
        )}
      >
        {initials(person.name)}
      </span>
      <div className="min-w-0 flex-1">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={rename}
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          aria-label={`Name of ${person.name}`}
          className="-mx-1 w-full rounded-md bg-transparent px-1 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          data-testid="person-name"
        />
        <p className="text-xs text-muted-foreground tabular-nums">
          {person.meetings} {person.meetings === 1 ? "meeting" : "meetings"} · {person.samples}{" "}
          {person.samples === 1 ? "voice sample" : "voice samples"}
        </p>
      </div>
      {confirmDelete ? (
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">
            Delete {person.name} and their voice data?
          </span>
          <Button
            size="sm"
            variant="destructive"
            onClick={() => run(() => call("delete_person", { id: person.id }))}
            data-testid="confirm-delete-person"
          >
            Delete
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(false)}>
            Cancel
          </Button>
        </div>
      ) : (
        <>
          {others.length > 0 && (
            <label className="relative flex items-center">
              <span className="sr-only">Merge {person.name} into</span>
              <select
                value=""
                onChange={(e) =>
                  e.target.value &&
                  run(() =>
                    call("merge_people", { keep: Number(e.target.value), merged: person.id }),
                  )
                }
                className="h-7 appearance-none rounded-md border bg-background pr-7 pl-2 text-xs text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                data-testid="merge-person"
              >
                <option value="">Same person as…</option>
                {others.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.name}
                  </option>
                ))}
              </select>
              <ChevronDown className="pointer-events-none absolute right-2 size-3.5 text-muted-foreground" />
            </label>
          )}
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label={`Delete ${person.name}`}
            onClick={() => setConfirmDelete(true)}
            data-testid="delete-person"
          >
            <Trash2 className="text-muted-foreground" />
          </Button>
        </>
      )}
    </div>
  );
}
