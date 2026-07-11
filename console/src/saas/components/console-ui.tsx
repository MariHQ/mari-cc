import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Search, ChevronUp, ChevronDown, ChevronsUpDown, ChevronLeft, ChevronRight, Inbox, X } from "lucide-react";

/* Console primitives — "adapted blueprint": the landing's ink/paper/mono
   vocabulary tuned for app density. Hairline ink borders carry separation
   (no drop shadows), corners sit at 4–6px, and the chrome layer (column
   headers, labels, counts, badges) is JetBrains Mono. */

export const card = "bg-paper rounded-md border border-ink/15";

/* Shared focus treatment — keyboard users need to see where they are. */
export const focusRing =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-biscay-2/70 focus-visible:ring-offset-1";

export function Page({ title, subtitle, kicker, actions, children }: { title: string; subtitle: string; kicker?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <div className="font-display text-ink bg-paper min-h-full p-4 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          {kicker && (
            <div className="flex items-center gap-2 mb-1.5 font-term text-[10.5px] font-medium uppercase tracking-[0.18em] text-biscay-2">
              <span className="inline-block w-[7px] h-[7px] bg-biscay-2" aria-hidden />
              {kicker}
            </div>
          )}
          <h3 className="text-[22px] font-bold tracking-[-0.015em] text-ink">{title}</h3>
          <p className="text-[13px] text-ink/60 mt-1 max-w-[680px]">{subtitle}</p>
        </div>
        {actions && <div className="flex items-center gap-2">{actions}</div>}
      </div>
      {children}
    </div>
  );
}

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

const thClass = "font-term font-medium text-[11px] uppercase tracking-[0.08em] text-ink/60";

export function Table({ title, count, head, footer, minW = 700, children }: { title?: string; count?: number; head: string[]; footer?: ReactNode; minW?: number; children: ReactNode }) {
  return (
    <div className={`${card} mt-5 overflow-hidden`}>
      {title && (
        <div className="flex items-center gap-2 px-4 pt-4 pb-3">
          <h4 className="text-[15px] font-semibold text-ink">{title}</h4>
          {count != null && <span className="font-term text-[11px] font-medium text-ink/60 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5">{count}</span>}
        </div>
      )}
      <div className="overflow-x-auto">
        <table className="w-full text-left border-collapse" style={{ minWidth: minW }}>
          <thead>
            <tr>
              {head.map((h) => <th key={h} className={`${thClass} px-4 py-2.5 border-y border-ink/10`}>{h}</th>)}
            </tr>
          </thead>
          <tbody>{children}</tbody>
        </table>
      </div>
      {footer}
    </div>
  );
}

export const btn = `inline-flex items-center gap-1.5 h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] font-medium text-ink/80 hover:border-ink/45 hover:text-ink transition-colors ${focusRing}`;
export const btnPrimary = `inline-flex items-center gap-1.5 h-9 px-3.5 rounded-[4px] bg-biscay text-white text-[13px] font-semibold hover:bg-biscay-2 transition-colors ${focusRing}`;
export const btnDanger = `inline-flex items-center gap-1.5 h-9 px-3 rounded-[4px] border border-espelette/40 bg-paper text-[13px] font-medium text-espelette hover:bg-espelette/[0.06] hover:border-espelette transition-colors ${focusRing}`;

/* ── DataTable: search, sort, filter, paginate, empty state ────────────── */
export type Column<T> = {
  key: string;
  header: string;
  sortable?: boolean;
  sort?: (row: T) => string | number;
  render: (row: T) => ReactNode;
  align?: "right";
  cell?: string;
};

export function DataTable<T>({
  title, count, rows, columns, rowKey, search, searchPlaceholder = "Search…",
  facet, onRowClick, pageSize = 8, minW = 720, empty = "No results",
}: {
  title?: string;
  count?: number;
  rows: T[];
  columns: Column<T>[];
  rowKey: (row: T) => string;
  search?: (row: T) => string;
  searchPlaceholder?: string;
  facet?: { label: string; get: (row: T) => string };
  onRowClick?: (row: T) => void;
  pageSize?: number;
  minW?: number;
  empty?: string;
}) {
  const [query, setQuery] = useState("");
  const [facetVal, setFacetVal] = useState("");
  const [sortKey, setSortKey] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");
  const [page, setPage] = useState(0);

  const facetOptions = useMemo(() => (facet ? Array.from(new Set(rows.map(facet.get))) : []), [rows, facet]);

  const filtered = useMemo(() => {
    let r = rows;
    if (query && search) { const q = query.toLowerCase(); r = r.filter((x) => search(x).toLowerCase().includes(q)); }
    if (facet && facetVal) r = r.filter((x) => facet.get(x) === facetVal);
    const col = columns.find((c) => c.key === sortKey);
    if (col?.sort) {
      const s = col.sort;
      r = [...r].sort((a, b) => {
        const av = s(a), bv = s(b);
        const cmp = typeof av === "number" && typeof bv === "number" ? av - bv : String(av).localeCompare(String(bv));
        return sortDir === "asc" ? cmp : -cmp;
      });
    }
    return r;
  }, [rows, query, facet, facetVal, sortKey, sortDir, columns, search]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize));
  const cur = Math.min(page, pageCount - 1);
  const pageRows = filtered.slice(cur * pageSize, cur * pageSize + pageSize);

  const toggleSort = (key: string) => {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else { setSortKey(key); setSortDir("asc"); }
    setPage(0);
  };

  return (
    <div className={`${card} mt-5 overflow-hidden`}>
      <div className="flex flex-wrap items-center gap-2 px-4 py-3 border-b border-ink/10">
        {title && (
          <div className="flex items-center gap-2 mr-1">
            <h4 className="text-[15px] font-semibold text-ink">{title}</h4>
            {count != null && <span className="font-term text-[11px] font-medium text-ink/60 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5">{count}</span>}
          </div>
        )}
        <div className="flex items-center gap-2 ml-auto">
          {search && (
            <div className="flex items-center gap-1.5 h-8 px-2.5 rounded-[4px] border border-ink/20 bg-paper focus-within:border-biscay-2 focus-within:ring-1 focus-within:ring-biscay-2/40">
              <Search size={13} className="text-ink/50" />
              <input value={query} onChange={(e) => { setQuery(e.target.value); setPage(0); }} placeholder={searchPlaceholder} className="w-[130px] sm:w-[170px] bg-transparent text-[12.5px] text-ink placeholder:text-ink/45 outline-none" />
            </div>
          )}
          {facet && (
            <select value={facetVal} onChange={(e) => { setFacetVal(e.target.value); setPage(0); }} className={`h-8 px-2.5 rounded-[4px] border border-ink/20 bg-paper text-[12.5px] text-ink/75 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40`}>
              <option value="">All {facet.label}</option>
              {facetOptions.map((o) => <option key={o} value={o}>{o}</option>)}
            </select>
          )}
        </div>
      </div>

      {pageRows.length === 0 ? (
        <div className="grid place-items-center py-16 text-center">
          <Inbox size={24} className="text-ink/25" />
          <p className="mt-2 text-[13px] text-ink/60">{query || facetVal ? "No matches. Try clearing filters." : empty}</p>
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse" style={{ minWidth: minW }}>
            <thead>
              <tr>
                {columns.map((c) => (
                  <th key={c.key} className={`${thClass} px-4 py-2.5 border-b border-ink/10 ${c.align === "right" ? "text-right" : ""}`}>
                    {c.sortable ? (
                      <button onClick={() => toggleSort(c.key)} className={`inline-flex items-center gap-1 uppercase hover:text-ink rounded-[3px] ${focusRing}`}>
                        {c.header}
                        {sortKey === c.key ? (sortDir === "asc" ? <ChevronUp size={12} /> : <ChevronDown size={12} />) : <ChevronsUpDown size={12} className="text-ink/30" />}
                      </button>
                    ) : c.header}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {pageRows.map((row) => (
                <tr key={rowKey(row)} onClick={onRowClick ? () => onRowClick(row) : undefined} className={`border-b border-ink/10 last:border-0 ${onRowClick ? "cursor-pointer hover:bg-flysch/50 group" : ""}`}>
                  {columns.map((c) => <td key={c.key} className={`px-4 py-3 ${c.align === "right" ? "text-right" : ""} ${c.cell ?? ""}`}>{c.render(row)}</td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {filtered.length > pageSize && (
        <div className="flex items-center justify-between px-4 py-2.5 border-t border-ink/10 font-term text-[11.5px] text-ink/60">
          <span>{cur * pageSize + 1}–{Math.min((cur + 1) * pageSize, filtered.length)} of {filtered.length}</span>
          <div className="flex items-center gap-1">
            <button disabled={cur === 0} onClick={() => setPage((p) => Math.max(0, p - 1))} aria-label="Previous page" className={`grid place-items-center w-7 h-7 rounded-[4px] border border-ink/20 text-ink/70 disabled:opacity-40 hover:bg-flysch ${focusRing}`}><ChevronLeft size={14} /></button>
            <button disabled={cur >= pageCount - 1} onClick={() => setPage((p) => Math.min(pageCount - 1, p + 1))} aria-label="Next page" className={`grid place-items-center w-7 h-7 rounded-[4px] border border-ink/20 text-ink/70 disabled:opacity-40 hover:bg-flysch ${focusRing}`}><ChevronRight size={14} /></button>
          </div>
        </div>
      )}
    </div>
  );
}

/* ── Drawer: right-side detail slide-over ──────────────────────────────── */
const FOCUSABLE = 'a[href], button:not([disabled]), textarea, input, select, [tabindex]:not([tabindex="-1"])';

export function Drawer({ open, onClose, title, subtitle, icon, footer, children }: { open: boolean; onClose: () => void; title: string; subtitle?: string; icon?: ReactNode; footer?: ReactNode; children: ReactNode }) {
  const panelRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key !== "Tab") return;
      const panel = panelRef.current;
      if (!panel) return;
      const els = panel.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (els.length === 0) { e.preventDefault(); return; }
      const first = els[0];
      const last = els[els.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === panel)) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && active === last) { e.preventDefault(); first.focus(); }
    };

    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      previouslyFocused?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-[60] font-display">
      <div className="absolute inset-0 bg-ink/30 drawer-overlay" onClick={onClose} />
      <aside
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        className="absolute right-0 top-0 h-full w-full max-w-[460px] bg-paper border-l border-ink/15 shadow-2xl flex flex-col text-ink drawer-panel outline-none"
      >
        <header className="shrink-0 h-14 px-5 flex items-center gap-3 border-b border-ink/10">
          {icon}
          <div className="min-w-0 flex-1">
            <div className="text-[14px] font-semibold text-ink truncate">{title}</div>
            {subtitle && <div className="font-term text-[11px] uppercase tracking-[0.08em] text-ink/55 truncate">{subtitle}</div>}
          </div>
          <button onClick={onClose} className={`grid place-items-center w-8 h-8 rounded-[4px] text-ink/60 hover:bg-flysch hover:text-ink ${focusRing}`} aria-label="Close"><X size={16} /></button>
        </header>
        <div className="flex-1 overflow-y-auto p-5">{children}</div>
        {footer && <footer className="shrink-0 p-4 border-t border-ink/10 flex gap-2">{footer}</footer>}
      </aside>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="py-2.5 border-b border-ink/10 last:border-0">
      <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55">{label}</div>
      <div className="mt-1 text-[13px] text-ink/90">{children}</div>
    </div>
  );
}
