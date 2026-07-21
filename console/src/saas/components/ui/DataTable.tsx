import { useMemo, useState, type ReactNode } from "react";
import { ChevronDown, ChevronLeft, ChevronRight, ChevronsUpDown, ChevronUp, Inbox, Search } from "lucide-react";
import { card } from "./card";
import { focusRing } from "./focusRing";

const thClass = "font-term font-medium text-[11px] uppercase tracking-[0.08em] text-ink/60";

export type Column<T> = {
  key: string;
  header: string;
  sortable?: boolean;
  sort?: (row: T) => string | number;
  render: (row: T) => ReactNode;
  align?: "right";
  cell?: string;
};

/* ── DataTable: search, sort, filter, paginate, empty state ────────────── */
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
