import {
  LayoutGrid,
  BookMarked,
  LayoutTemplate,
  Languages,
  ScrollText,
  SpellCheck,
  BellRing,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";

// Sidebar sections. Each id maps to a self-contained group component rendered
// by Console.tsx. Only surfaces the local mari-cc backend actually supports are
// listed here — there is no auth, billing, org, or hosted governance.
export type StepId =
  | "overview"
  | "glossary"
  | "templates"
  | "localization"
  | "detect"
  | "rules"
  | "nudges"
  | "config";

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
    description: "Your deterministic prose system at a glance." },

  { id: "glossary", name: "Glossary", icon: BookMarked, group: "Curation",
    description: "Preferred terms and their variants, from STYLE.md. Generate a STYLE.md here." },
  { id: "templates", name: "Templates", icon: LayoutTemplate, group: "Curation",
    description: "Document archetypes (runbook, ADR, RFC, …). Scaffold a new doc from a template." },

  { id: "localization", name: "Localization", icon: Languages, group: "Docs",
    description: "Translation coverage across languages — which docs are localized, and which are stale." },
  { id: "detect", name: "Detector", icon: SpellCheck, group: "Governance",
    description: "Run the deterministic prose detector on text or a file — findings, slop score, and one-click waivers." },
  { id: "rules", name: "Rules", icon: ScrollText, group: "Governance",
    description: "Edit-notify rules and the detector: waivers, zero-tolerance, and the full rule catalog." },
  { id: "nudges", name: "Nudges", icon: BellRing, group: "Governance",
    description: "Hand-declared maintenance couplings: when this changes, remember to update that." },

  { id: "config", name: "Config", icon: SlidersHorizontal, pinBottom: true,
    description: "Effective repository configuration, clustered by area." },
];

export const stepById = (id: StepId): Step => STEPS.find((s) => s.id === id) ?? STEPS[0];
