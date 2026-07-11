import { useState, type FormEvent } from "react";
import { api, type SearchHit } from "@saas/lib/client";
import { Page, Badge, card, btnPrimary, focusRing } from "../console-ui";
import { Search as SearchIcon, Loader2, ArrowRight } from "lucide-react";

/* Format an ISO timestamp as a compact relative string ("3d ago"). */
function relTime(iso: string | null): string {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const diff = Date.now() - t;
  const sec = Math.round(diff / 1000);
  if (sec < 0) return "just now";
  if (sec < 60) return `${sec}s ago`;
  const min = Math.round(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.round(hr / 24);
  if (day < 30) return `${day}d ago`;
  const mo = Math.round(day / 30);
  if (mo < 12) return `${mo}mo ago`;
  return `${Math.round(mo / 12)}y ago`;
}

/* Map a tag/status label to a Badge tone. */
function tagTone(tag: string | null | undefined): string {
  if (!tag) return "neutral";
  switch (tag.toLowerCase()) {
    case "canonical": return "ok";
    case "draft": return "neutral";
    case "stale": return "attention";
    case "deprecated": return "blocked";
    case "internal":
    case "customer-facing":
    case "needs-review": return "info";
    default: return "neutral";
  }
}

function HitCard({ hit }: { hit: SearchHit }) {
  return (
    <div className={`${card} p-4`}>
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-term text-[12.5px] font-medium text-ink break-all">{hit.canonical_ref}</span>
            {hit.tag && <Badge label={hit.tag} tone={tagTone(hit.tag)} />}
          </div>
          {hit.title && <div className="mt-0.5 text-[13px] text-ink/80 truncate">{hit.title}</div>}
        </div>
        <span className="font-term text-[12px] font-medium text-biscay-2 tabular-nums shrink-0">{hit.score.toFixed(3)}</span>
      </div>

      <div className="mt-1.5 font-term text-[11px] text-ink/50 flex flex-wrap items-center gap-x-2 gap-y-0.5">
        {hit.heading_path && <span className="break-all">{hit.heading_path}</span>}
        <span className="whitespace-nowrap">L{hit.start_line}-{hit.end_line}</span>
        {hit.author && <span>· {hit.author}</span>}
        {hit.updated_at && <span>· {relTime(hit.updated_at)}</span>}
      </div>

      <pre className="mt-2.5 max-h-[5rem] overflow-auto whitespace-pre-wrap font-term text-[12px] leading-[1.55] text-ink/85 bg-flysch/50 rounded-[4px] p-2.5 border border-ink/10">{hit.text}</pre>

      {hit.replacement && (
        <div className="mt-2 flex items-center gap-1.5 font-term text-[11.5px] text-clay">
          <ArrowRight size={12} className="shrink-0" />
          <span className="break-all">{hit.replacement}</span>
        </div>
      )}

      {hit.matched_terms.length > 0 && (
        <div className="mt-2.5 flex flex-wrap gap-1">
          {hit.matched_terms.map((t, i) => (
            <span key={`${t}-${i}`} className="inline-flex items-center rounded-[3px] border border-ink/20 bg-ink/[0.04] px-1.5 py-[1.5px] font-term text-[10.5px] text-ink/65">{t}</span>
          ))}
        </div>
      )}
    </div>
  );
}

export function SearchGroup() {
  const [q, setQ] = useState("");
  const [k, setK] = useState(8);
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function run(e?: FormEvent) {
    e?.preventDefault();
    const term = q.trim();
    if (!term) return;
    setLoading(true);
    setError(null);
    api.search({ q: term, k }).then(
      (r) => { setHits(r.hits); setQuery(r.query); setLoading(false); },
      (err) => { setError(String(err?.message ?? err)); setLoading(false); },
    );
  }

  return (
    <Page title="Search" subtitle="Hybrid semantic + keyword search across everything." kicker="content">
      <form onSubmit={run} className={`${card} mt-5 p-3 flex flex-wrap items-center gap-2`}>
        <div className="flex items-center gap-2 flex-1 min-w-[220px] h-11 px-3 rounded-[4px] border border-ink/20 bg-paper focus-within:border-biscay-2 focus-within:ring-1 focus-within:ring-biscay-2/40">
          <SearchIcon size={16} className="text-ink/50 shrink-0" />
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Ask anything across your knowledge base…"
            autoFocus
            className="flex-1 bg-transparent text-[14px] text-ink placeholder:text-ink/45 outline-none"
          />
        </div>
        <label className="flex items-center gap-1.5 font-term text-[11px] uppercase tracking-[0.08em] text-ink/55">
          k
          <input
            type="number"
            min={1}
            max={50}
            value={k}
            onChange={(e) => setK(Math.max(1, Number(e.target.value) || 1))}
            className={`w-16 h-11 px-2 rounded-[4px] border border-ink/20 bg-paper text-[13px] text-ink/85 outline-none focus:border-biscay-2 ${focusRing}`}
          />
        </label>
        <button type="submit" className={`${btnPrimary} h-11`} disabled={loading || !q.trim()}>
          {loading ? <Loader2 size={15} className="animate-spin" /> : <SearchIcon size={15} />}
          Search
        </button>
      </form>

      <div className="mt-5">
        {loading ? (
          <div className={`${card} grid place-items-center py-20 text-ink/50`}>
            <Loader2 size={22} className="animate-spin" />
          </div>
        ) : error ? (
          <div className={`${card} p-6`}>
            <p className="text-[13px] text-espelette">Search failed. {error}</p>
          </div>
        ) : hits === null ? (
          <div className={`${card} grid place-items-center py-20 text-center`}>
            <SearchIcon size={24} className="text-ink/25" />
            <p className="mt-2 text-[13px] text-ink/55">Search your indexed documents by meaning or keyword.</p>
          </div>
        ) : hits.length === 0 ? (
          <div className={`${card} grid place-items-center py-20 text-center`}>
            <p className="text-[13px] text-ink/60">No matches — have you run a sync?</p>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="font-term text-[11.5px] text-ink/55">
              {hits.length} result{hits.length === 1 ? "" : "s"} for <span className="text-ink/80">"{query}"</span>
            </div>
            {hits.map((h) => <HitCard key={h.chunk_id} hit={h} />)}
          </div>
        )}
      </div>
    </Page>
  );
}
