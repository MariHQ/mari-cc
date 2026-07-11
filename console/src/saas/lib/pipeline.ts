import {
  LayoutGrid,
  Plug,
  Library,
  Search,
  Tag,
  Share2,
  BookMarked,
  ListChecks,
  LayoutTemplate,
  ScrollText,
  BellRing,
  SlidersHorizontal,
  Activity,
  Cloud,
  type LucideIcon,
} from "lucide-react";

// Sidebar sections. Each id maps to a self-contained group component rendered
// by Console.tsx. Only surfaces the local mari-cc backend actually supports are
// listed here — there is no auth, billing, org, or hosted governance.
export type StepId =
  | "overview"
  | "sources"
  | "documents"
  | "search"
  | "lineage"
  | "tags"
  | "glossary"
  | "facts"
  | "templates"
  | "rules"
  | "nudges"
  | "cloud"
  | "config"
  | "status";

export interface Step {
  id: StepId;
  name: string;
  icon: LucideIcon;
  description: string;
  group?: string;
  pinBottom?: boolean;
}

export const STEPS: Step[] = [
  { id: "overview", name: "Overview", icon: LayoutGrid,
    description: "Your knowledge base at a glance: documents, connectors, freshness, and recent syncs." },

  { id: "sources", name: "Sources", icon: Plug, group: "Content",
    description: "Connectors, what they track, and their sync status. Track refs and sync from here." },
  { id: "documents", name: "Documents", icon: Library, group: "Content",
    description: "Every indexed document. Read the body, see its chunks, tag it, trace its lineage." },
  { id: "search", name: "Search", icon: Search, group: "Content",
    description: "Hybrid semantic + keyword search across the whole knowledge base." },
  { id: "lineage", name: "Lineage", icon: Share2, group: "Content",
    description: "Span-to-span maintenance edges. Confirm or reject proposed couplings." },

  { id: "tags", name: "Tags", icon: Tag, group: "Curation",
    description: "Curation tags — canonical, stale, deprecated, draft — and the status vocabulary." },
  { id: "glossary", name: "Glossary", icon: BookMarked, group: "Curation",
    description: "Preferred terms and their variants, from STYLE.md. Generate a STYLE.md here." },
  { id: "facts", name: "Facts", icon: ListChecks, group: "Curation",
    description: "The claims ledger from FACTS.md that factcheck grounds against." },
  { id: "templates", name: "Templates", icon: LayoutTemplate, group: "Curation",
    description: "Document archetypes (runbook, ADR, RFC, …). Scaffold a new doc from a template." },

  { id: "rules", name: "Rules", icon: ScrollText, group: "Governance",
    description: "Edit-notify rules and the detector: waivers, zero-tolerance, and the full rule catalog." },
  { id: "nudges", name: "Nudges", icon: BellRing, group: "Governance",
    description: "Hand-declared maintenance couplings: when this changes, remember to update that." },

  { id: "cloud", name: "Cloud", icon: Cloud, pinBottom: true,
    description: "Team sharing: push and pull the knowledge base to an S3 (or git) warehouse." },
  { id: "config", name: "Config", icon: SlidersHorizontal, pinBottom: true,
    description: "Effective configuration, clustered by area. Edit repo or global values." },
  { id: "status", name: "Status", icon: Activity, pinBottom: true,
    description: "Workspace, embedding model, catalog, and cloud status." },
];

export const stepById = (id: StepId): Step => STEPS.find((s) => s.id === id) ?? STEPS[0];
