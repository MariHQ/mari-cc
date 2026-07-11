import { useEffect, useMemo, useState } from "react";
import { Languages, Check, FileText, Sparkles, ChevronRight, AlertTriangle } from "lucide-react";
import {
  api,
  type Localization,
  type LocalizationSource,
  type LocalizationCell,
  type CoverageResult,
} from "@saas/lib/client";
import { Page, Badge, card, btn, focusRing } from "../console-ui";
import { toast } from "../feedback";

const shortName = (p: string) => p.split("/").pop() || p;

/* ── A collapsible file viewer that lazy-loads content on first open. ─────── */
function FileViewer({ path }: { path: string }) {
  const [open, setOpen] = useState(false);
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  function toggle() {
    const next = !open;
    setOpen(next);
    if (next && content === null && !loading) {
      setLoading(true);
      api
        .repoFile(path)
        .then((r) => setContent(r.content + (r.truncated ? "\n\n… (truncated)" : "")))
        .catch((e) => setErr(e instanceof Error ? e.message : String(e)))
        .finally(() => setLoading(false));
    }
  }

  return (
    <div className="mt-2">
      <button
        onClick={toggle}
        className={`inline-flex items-center gap-1.5 font-term text-[11.5px] text-biscay-2 hover:underline ${focusRing}`}
      >
        <ChevronRight size={12} className={`transition-transform ${open ? "rotate-90" : ""}`} />
        <FileText size={12} />
        {open ? "Hide" : "View"} {shortName(path)}
      </button>
      {open && (
        <div className="mt-1.5 rounded-[4px] border border-ink/15 bg-flysch/40 overflow-hidden">
          {loading ? (
            <div className="px-3 py-4 text-[12px] text-ink/50">Loading…</div>
          ) : err ? (
            <div className="px-3 py-3 font-term text-[12px] text-espelette">{err}</div>
          ) : (
            <pre className="max-h-[280px] overflow-auto px-3 py-2.5 font-term text-[11.5px] leading-[1.55] text-ink/85 whitespace-pre-wrap">
              {content}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

/* ── Per-language panel: structure issues + on-demand attention coverage. ── */
function TranslationPanel({ source, lang, cell }: { source: string; lang: string; cell: LocalizationCell }) {
  const [cov, setCov] = useState<CoverageResult | null>(null);
  const [running, setRunning] = useState(false);

  async function runCoverage() {
    setRunning(true);
    try {
      const r = await api.localizationCoverage(source, cell.path);
      setCov(r);
      if (r.error) toast(r.error, "error");
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className={`${card} p-3.5`}>
      <div className="flex items-center gap-2 flex-wrap">
        <span className="font-term text-[12px] font-semibold uppercase tracking-[0.08em] text-ink">{lang}</span>
        <Badge label={cell.layout} tone="neutral" />
        {cell.stale && <Badge label="stale" tone="attention" />}
        {cell.issues.length === 0 ? (
          <Badge label="structure ok" tone="ok" />
        ) : (
          <Badge label={`${cell.issues.length} structure issue${cell.issues.length === 1 ? "" : "s"}`} tone="blocked" />
        )}
        <span className="ml-auto font-term text-[11px] text-ink/45 truncate max-w-[180px]" title={cell.path}>
          {cell.path}
        </span>
      </div>

      {/* Deterministic structural diff */}
      {cell.issues.length > 0 && (
        <ul className="mt-2.5 flex flex-col gap-1">
          {cell.issues.map((iss, i) => (
            <li key={i} className="flex items-start gap-1.5 text-[12.5px] text-ink/80">
              <AlertTriangle size={13} className="text-clay mt-0.5 shrink-0" />
              <span>{iss}</span>
            </li>
          ))}
        </ul>
      )}

      <FileViewer path={cell.path} />

      {/* Deep attention coverage (on demand) */}
      <div className="mt-3 border-t border-ink/10 pt-2.5">
        <div className="flex items-center gap-2">
          <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">Deep coverage</span>
          <button onClick={runCoverage} disabled={running} className={`${btn} h-7 px-2 text-[12px] disabled:opacity-60 ml-auto`}>
            <Sparkles size={12} />
            {running ? "Analyzing…" : cov ? "Re-run" : "Run"}
          </button>
        </div>
        {cov && !cov.error && cov.flagged.length === 0 && (
          <div className="mt-2 flex items-center gap-1.5 text-[12.5px] text-moss">
            <Check size={14} /> Prose coverage complete — nothing under-covered.
          </div>
        )}
        {cov && cov.flagged.length > 0 && (
          <>
            <div className="mt-2 flex flex-col gap-1.5">
              {cov.flagged.map((f, i) => (
                <div key={i} className="rounded-[4px] border border-clay/25 bg-clay/[0.05] px-2.5 py-1.5">
                  <div className="flex items-center gap-2 font-term text-[11px] text-ink/55">
                    <span className="text-clay font-semibold">{Math.round(f.score * 100)}% covered</span>
                    <span>≈ L{f.line}</span>
                  </div>
                  <div className="mt-0.5 text-[12.5px] text-ink/80">{f.text}</div>
                </div>
              ))}
            </div>
            <p className="mt-1.5 font-term text-[11px] text-ink/45">
              Barely-covered source passages — leads, not verdicts; idiom legitimately drifts.
            </p>
          </>
        )}
        {!cov && (
          <p className="mt-1.5 font-term text-[11px] text-ink/45">
            Runs the local attention model to find source passages the translation barely covers.
          </p>
        )}
      </div>
    </div>
  );
}

/* ── A compact language status chip for the collapsed card header. ───────── */
function LangChip({ lang, cell }: { lang: string; cell: LocalizationCell | undefined }) {
  if (!cell) {
    return (
      <span className="inline-flex items-center gap-1 font-term text-[11px] text-ink/35 border border-ink/10 rounded-[3px] px-1.5 py-0.5">
        {lang} —
      </span>
    );
  }
  const tone = cell.issues.length > 0 ? "blocked" : cell.stale ? "attention" : "ok";
  const label = cell.issues.length > 0 ? `${cell.issues.length}` : cell.stale ? "stale" : "";
  const cls =
    tone === "blocked"
      ? "text-espelette border-espelette/30 bg-espelette/[0.06]"
      : tone === "attention"
      ? "text-clay border-clay/35 bg-clay/[0.07]"
      : "text-moss border-moss/30 bg-moss/[0.06]";
  return (
    <span className={`inline-flex items-center gap-1 font-term text-[11px] rounded-[3px] border px-1.5 py-0.5 ${cls}`}>
      {lang}
      {tone === "ok" ? <Check size={11} /> : <span>{label}</span>}
    </span>
  );
}

/* ── A collapsible per-source card (replaces the old modal). ─────────────── */
function SourceCard({
  src,
  languages,
  sourceLang,
  open,
  onToggle,
}: {
  src: LocalizationSource;
  languages: string[];
  sourceLang: string;
  open: boolean;
  onToggle: () => void;
}) {
  const missing = languages.filter((l) => !src.translations[l]);
  return (
    <div className={`${card} overflow-hidden`}>
      <button
        onClick={onToggle}
        className={`w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-flysch/40 transition-colors ${focusRing}`}
      >
        <ChevronRight size={15} className={`text-ink/40 shrink-0 transition-transform ${open ? "rotate-90" : ""}`} />
        <span className="min-w-0 flex-1">
          <span className="text-[13.5px] font-medium text-ink truncate block">{shortName(src.source)}</span>
          <span className="font-term text-[11px] text-ink/45 truncate block">{src.source}</span>
        </span>
        <span className="flex items-center gap-1.5 flex-wrap justify-end">
          {languages.map((l) => (
            <LangChip key={l} lang={l} cell={src.translations[l]} />
          ))}
        </span>
      </button>

      {open && (
        <div className="border-t border-ink/10 p-4 flex flex-col gap-4 bg-paper">
          <div>
            <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55 mb-1">
              Source ({sourceLang})
            </div>
            <FileViewer path={src.source} />
          </div>
          <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55">
            Translations ({Object.keys(src.translations).length})
          </div>
          {languages
            .filter((l) => src.translations[l])
            .map((l) => (
              <TranslationPanel key={l} source={src.source} lang={l} cell={src.translations[l]} />
            ))}
          {missing.length > 0 && (
            <div className="text-[12.5px] text-ink/55">
              Missing:{" "}
              {missing.map((l) => (
                <span key={l} className="font-term text-ink/70 mr-1.5">
                  {l}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function LocalizationGroup() {
  const [data, setData] = useState<Localization | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  function reload() {
    setLoading(true);
    setError(null);
    api
      .localization()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  const stats = useMemo(() => {
    if (!data) return { localized: 0, stale: 0, issues: 0 };
    let stale = 0;
    let issues = 0;
    for (const s of data.sources) {
      for (const lang of Object.keys(s.translations)) {
        const c = s.translations[lang];
        if (c.stale) stale++;
        issues += c.issues.length;
      }
    }
    return { localized: data.sources.length, stale, issues };
  }, [data]);

  const toggle = (key: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  const allOpen = !!data && data.sources.length > 0 && data.sources.every((s) => expanded.has(s.source));
  const toggleAll = () =>
    setExpanded(allOpen ? new Set() : new Set(data?.sources.map((s) => s.source) ?? []));

  return (
    <Page
      title="Localization"
      subtitle="Translation coverage, structural drift, and deep attention coverage — expand a doc to explore inline."
      kicker="docs"
      actions={
        data && data.sources.length > 0 ? (
          <button onClick={toggleAll} className={btn}>
            {allOpen ? "Collapse all" : "Expand all"}
          </button>
        ) : undefined
      }
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load localization</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && data.languages.length === 0 && (
        <div className={`${card} mt-5 p-6 text-center`}>
          <Languages size={22} className="mx-auto text-ink/25" />
          <div className="mx-auto mt-2 max-w-[460px] text-[13px] text-ink/70">
            No translations found. Mari detects <span className="font-term">.es.md</span>-style suffixes,{" "}
            <span className="font-term">/es/</span> language directories, and Hugo/Docusaurus layouts. Add a
            translated sibling of a doc and it appears here.
          </div>
          <div className="mt-1 font-term text-[11.5px] text-ink/45">source languages: {data.sourceLangs.join(", ")}</div>
        </div>
      )}

      {data && !error && data.languages.length > 0 && (
        <>
          <div className="mt-5 grid grid-cols-2 sm:grid-cols-4 gap-4 max-w-[720px]">
            <Stat label="Languages" value={data.languages.length} />
            <Stat label="Localized docs" value={stats.localized} />
            <Stat label="Stale" value={stats.stale} tone={stats.stale > 0 ? "warn" : "ok"} />
            <Stat label="Structure issues" value={stats.issues} tone={stats.issues > 0 ? "bad" : "ok"} />
          </div>

          <div className="mt-5 flex flex-col gap-2.5">
            {data.sources.map((s) => (
              <SourceCard
                key={s.source}
                src={s}
                languages={data.languages}
                sourceLang={data.sourceLangs[0] ?? "en"}
                open={expanded.has(s.source)}
                onToggle={() => toggle(s.source)}
              />
            ))}
          </div>
          <p className="mt-3 font-term text-[11.5px] text-ink/45">
            Chips: <span className="text-moss">lang ✓</span> in sync · <span className="text-clay">stale</span> source
            newer · <span className="text-espelette">N</span> structure issues · lang — missing.
          </p>
        </>
      )}
    </Page>
  );
}

function Stat({ label, value, tone }: { label: string; value: number; tone?: "ok" | "warn" | "bad" }) {
  const color = tone === "bad" ? "text-espelette" : tone === "warn" ? "text-clay" : tone === "ok" ? "text-moss" : "text-ink";
  return (
    <div className={`${card} px-4 py-3`}>
      <div className="font-term text-[10px] uppercase tracking-[0.12em] text-ink/50">{label}</div>
      <div className={`mt-1 font-term text-[24px] font-semibold ${color}`}>{value}</div>
    </div>
  );
}
