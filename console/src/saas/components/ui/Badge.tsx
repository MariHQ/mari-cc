/* Status tones — one semantic scale for the whole console:
     ok        things that are healthy/approved/synced/live
     attention pending, syncing, in review, needs update
     blocked   failing, flagged, stale, needs evidence
     info      informational/active-by-design
     neutral   drafts and everything else
   Legacy tone names used across pages are aliased below. */
const TONE: Record<string, string> = {
  ok: "text-moss border-moss/30 bg-moss/[0.06]",
  attention: "text-clay border-clay/35 bg-clay/[0.07]",
  blocked: "text-espelette border-espelette/30 bg-espelette/[0.06]",
  info: "text-biscay-2 border-biscay-2/35 bg-biscay-2/[0.06]",
  neutral: "text-ink/70 border-ink/20 bg-ink/[0.04]",
};
const TONE_ALIAS: Record<string, string> = {
  approved: "ok", good: "ok",
  pending: "attention", review: "attention", warn: "attention",
  flagged: "blocked", bad: "blocked", error: "blocked",
  primary: "info", technical: "info",
  muted: "neutral",
};

export function Badge({ label, tone = "neutral" }: { label: string; tone?: string }) {
  const t = TONE[tone] ?? TONE[TONE_ALIAS[tone] ?? "neutral"] ?? TONE.neutral;
  return (
    <span className={`inline-flex items-center rounded-[3px] border px-1.5 py-[2.5px] font-term text-[11px] font-medium whitespace-nowrap ${t}`}>
      {label}
    </span>
  );
}
