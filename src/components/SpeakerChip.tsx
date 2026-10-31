import { Check } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { speakerBg } from "@/lib/format";
import { call, type Person, type SourceKind, type VoiceView } from "@/lib/ipc";
import { cn } from "@/lib/utils";

const CHIP = "rounded-full px-2 py-0.5 font-medium text-foreground";

/**
 * Who spoke (doc-3): "Me" (renamable), a named person in their colour, "Speaker N", or the
 * source of a live caption. A speaker chip opens a popover to name the speaker, confirm a
 * suggested name, or merge them into another speaker of the meeting (VRT-18).
 */
export function SpeakerChip({
  meetingId,
  label,
  kind,
  meName,
  voices,
  onRenameMe,
}: {
  meetingId: number;
  label: string | null;
  kind: SourceKind;
  meName: string;
  voices: VoiceView[];
  onRenameMe: (name: string) => void;
}) {
  const [editingMe, setEditingMe] = useState(false);
  const [open, setOpen] = useState(false);

  if (label === "me" || (label === null && kind === "mic")) {
    if (editingMe) {
      return (
        <input
          autoFocus
          defaultValue={meName}
          aria-label="Your name"
          className={cn(CHIP, "w-32 bg-muted ring-2 ring-ring/50 outline-none")}
          onBlur={(e) => {
            setEditingMe(false);
            onRenameMe(e.currentTarget.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            if (e.key === "Escape") setEditingMe(false);
          }}
          data-testid="me-name-input"
        />
      );
    }
    return (
      <button
        type="button"
        className={cn(CHIP, "bg-muted hover:bg-muted/70")}
        title="Rename"
        onClick={() => setEditingMe(true)}
        data-testid="speaker-chip"
        data-speaker="me"
      >
        {meName}
      </button>
    );
  }
  if (label === null) {
    return <span className={cn(CHIP, "bg-muted")}>System audio</span>;
  }

  const voice = voices.find((v) => v.label === label);
  const person = voice?.speaker ?? null;
  const suggestion = !person ? (voice?.suggestion ?? null) : null;
  return (
    <span className="relative">
      <button
        type="button"
        className={cn(CHIP, speakerBg(person ? person.color : Number(label)), "hover:opacity-80")}
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        data-testid="speaker-chip"
        data-speaker={label}
        data-person={person?.name ?? ""}
      >
        {person ? person.name : `Speaker ${label}`}
        {suggestion && (
          <span className="ml-1 font-normal text-foreground/70" data-testid="suggestion">
            · {suggestion.name}?
          </span>
        )}
      </button>
      {open && (
        <SpeakerPopover
          meetingId={meetingId}
          label={label}
          person={person}
          suggestion={suggestion}
          others={voices.filter((v) => v.label !== label)}
          onClose={() => setOpen(false)}
        />
      )}
    </span>
  );
}

function SpeakerPopover({
  meetingId,
  label,
  person,
  suggestion,
  others,
  onClose,
}: {
  meetingId: number;
  label: string;
  person: VoiceView["speaker"];
  suggestion: VoiceView["suggestion"];
  others: VoiceView[];
  onClose: () => void;
}) {
  const [name, setName] = useState(person?.name ?? "");
  const [people, setPeople] = useState<Person[]>([]);
  const [error, setError] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    call("list_people").then(setPeople, () => {});
    // The chip can sit at the edge of the scrolling transcript: show the whole popover.
    ref.current?.scrollIntoView({ block: "nearest" });
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [onClose]);

  const run = async (action: () => Promise<unknown>) => {
    setError(null);
    try {
      await action();
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  const save = () => run(() => call("name_speaker", { meetingId, label, name }));
  const otherName = (v: VoiceView) => v.speaker?.name ?? `Speaker ${v.label}`;

  return (
    <div
      ref={ref}
      role="dialog"
      aria-label={`Who is Speaker ${label}?`}
      className="absolute top-full left-0 z-20 mt-2 flex w-72 flex-col gap-3 rounded-lg border bg-popover p-3 text-sm text-popover-foreground shadow-lg"
      data-testid="speaker-popover"
    >
      {suggestion && (
        <div className="flex items-center justify-between gap-2">
          <span>
            Is this <span className="font-medium">{suggestion.name}</span>?
          </span>
          <Button
            size="sm"
            onClick={() => run(() => call("confirm_suggestion", { meetingId, label }))}
            data-testid="confirm-suggestion"
          >
            <Check /> Yes
          </Button>
        </div>
      )}
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim()) save();
        }}
      >
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          list="known-people"
          placeholder="Name"
          aria-label="Name"
          className="min-w-0 flex-1 rounded-md border bg-background px-2 py-1 outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          data-testid="speaker-name-input"
        />
        <datalist id="known-people">
          {people.map((p) => (
            <option key={p.id} value={p.name} />
          ))}
        </datalist>
        <Button size="sm" type="submit" disabled={!name.trim()} data-testid="save-speaker-name">
          Save
        </Button>
      </form>
      {others.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-xs text-muted-foreground">Same person as</span>
          <div className="flex flex-wrap gap-1">
            {others.map((v) => (
              <button
                key={v.label}
                type="button"
                className="rounded-full border px-2 py-0.5 text-xs hover:bg-muted"
                onClick={() =>
                  run(() => call("merge_labels", { meetingId, from: label, into: v.label }))
                }
                data-testid="merge-into"
                data-into={v.label}
              >
                {otherName(v)}
              </button>
            ))}
          </div>
        </div>
      )}
      <p className="text-xs text-muted-foreground">
        Naming keeps a voice sample on this computer, so Veritas recognizes this person in later
        meetings.
      </p>
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
