import { Users } from "lucide-react";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";

/** Speakers (doc-3 screen 4). The voice registry comes with VRT-18. */
export function Speakers() {
  return (
    <>
      <PageHeader title="Speakers" />
      <EmptyState icon={Users} title="No known voices yet">
        Name a speaker in a transcript and Veritas recognizes them in later meetings. Voice data
        stays on this computer.
      </EmptyState>
    </>
  );
}
