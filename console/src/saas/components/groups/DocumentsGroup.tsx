import { useEffect, useState } from "react";
import { api, type DocumentRow, type DocumentDetail } from "@saas/lib/client";
import { Page, DataTable, Drawer, Field, Badge, Table, btn, btnDanger, card, focusRing, type Column } from "../console-ui";
import { toast } from "../feedback";
import { FileText, Loader2, ExternalLink, ArrowLeftRight } from "lucide-react";

/* Format an ISO timestamp as a compact relative string ("3d ago").
   Guards nulls and unparseable values. */
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
export function tagTone(tag: string | null | undefined): string {
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

function docTitle(d: DocumentRow): string {
  return d.title || d.path || d.canonical_ref;
}

function docSubRef(d: DocumentRow): string | null {
  const title = d.title || d.path || d.canonical_ref;
  const sub = d.path || d.canonical_ref;
  return sub && sub !== title ? sub : null;
}

/* ── Detail drawer body: fetch + render one document ───────────────────── */
function DocDetail({ row, statuses, onChanged }: { row: DocumentRow; statuses: string[]; onChanged: () => void }) {
  const [detail, setDetail] = useState<DocumentDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  function load() {
    setLoading(true);
    setError(null);
    api.document(row.doc_id).then(
      (d) => { setDetail(d); setStatus(d.document.tag || statuses[0] || ""); setLoading(false); },
      (e) => { setError(String(e?.message ?? e)); setLoading(false); },
    );
  }

  useEffect(() => { load(); /* eslint-disable-next-line */ }, [row.doc_id]);

  if (loading) {
    return (
      <div className="grid place-items-center py-16 text-ink/50">
        <Loader2 size={22} className="animate-spin" />
      </div>
    );
  }
  if (error || !detail) {
    return <div className="py-8 text-[13px] text-espelette">Failed to load document. {error}</div>;
  }

  const doc = detail.document;

  async function apply() {
    if (!status) return;
    setBusy(true);
    try {
      await api.applyTag(doc.canonical_ref, status);
      toast(`Tagged as ${status}`, "success");
      load();
      onChanged();
    } catch (e: any) {
      toast(`Failed to tag: ${e?.message ?? e}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setBusy(true);
    try {
      await api.removeTag(doc.canonical_ref);
      toast("Tag removed", "success");
      load();
      onChanged();
    } catch (e: any) {
      toast(`Failed to remove tag: ${e?.message ?? e}`, "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-5">
      {/* Fields */}
      <div>
        <Field label="Source"><Badge label={doc.provider} tone="neutral" /></Field>
        <Field label="Kind">{doc.kind}</Field>
        <Field label="Author">{doc.author_name || "—"}</Field>
        <Field label="Created">{relTime(doc.created_at)}</Field>
        <Field label="Updated">{relTime(doc.updated_at)}</Field>
        <Field label="Canonical ref"><span className="font-term text-[12px] break-all">{doc.canonical_ref}</span></Field>
        <Field label="URL">
          {doc.url ? (
            <a href={doc.url} target="_blank" rel="noreferrer" className={`inline-flex items-center gap-1 text-biscay-2 hover:underline ${focusRing} rounded-[3px]`}>
              <span className="truncate max-w-[320px]">{doc.url}</span><ExternalLink size={12} />
            </a>
          ) : "—"}
        </Field>
      </div>

      {/* Tag control */}
      <div className={`${card} p-3.5`}>
        <div className="flex items-center gap-2">
          <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55">Tag</div>
          {doc.tag ? <Badge label={doc.tag} tone={tagTone(doc.tag)} /> : <span className="text-[12px] text-ink/45">Untagged</span>}
        </div>
        {doc.tagNote && <p className="mt-1.5 text-[12px] text-ink/70">{doc.tagNote}</p>}
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className="h-9 px-2.5 rounded-[4px] border border-ink/20 bg-paper text-[12.5px] text-ink/80 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40"
          >
            {statuses.map((s) => <option key={s} value={s}>{s}</option>)}
          </select>
          <button className={btn} onClick={apply} disabled={busy || !status}>Apply</button>
          {doc.tag && <button className={btnDanger} onClick={remove} disabled={busy}>Remove</button>}
        </div>
      </div>

      {/* Body */}
      <div>
        <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55 mb-1.5">Body</div>
        <div className={`${card} p-3`}>
          <pre className="max-h-[360px] overflow-auto whitespace-pre-wrap font-term text-[12px] leading-[1.55] text-ink/85">{doc.body || "(empty)"}</pre>
        </div>
      </div>

      {/* Chunks */}
      {detail.chunks.length > 0 && (
        <Table title="Chunks" count={detail.chunks.length} head={["#", "Heading", "Lines", "Tokens"]} minW={360}>
          {detail.chunks.map((c) => (
            <tr key={c.chunk_id} className="border-b border-ink/10 last:border-0">
              <td className="px-4 py-2 font-term text-[12px] text-ink/70">{c.chunk_index}</td>
              <td className="px-4 py-2 text-[12.5px] text-ink/85">{c.heading_path || "—"}</td>
              <td className="px-4 py-2 font-term text-[12px] text-ink/70 whitespace-nowrap">L{c.start_line}-{c.end_line}</td>
              <td className="px-4 py-2 font-term text-[12px] text-ink/70">{c.token_count}</td>
            </tr>
          ))}
        </Table>
      )}

      {/* Lineage */}
      <div>
        <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55 mb-1.5">Lineage</div>
        {detail.lineage.length > 0 ? (
          <ul className="space-y-2">
            {detail.lineage.map((e) => (
              <li key={e.id} className={`${card} p-2.5`}>
                <div className="flex items-center gap-2 flex-wrap">
                  <Badge label={e.rel} tone="info" />
                  <Badge label={e.status} tone={tagTone(e.status)} />
                </div>
                <div className="mt-1.5 flex items-center gap-1.5 font-term text-[11.5px] text-ink/70">
                  <span className="break-all">{e.fromRef}</span>
                  <ArrowLeftRight size={12} className="shrink-0 text-ink/40" />
                  <span className="break-all">{e.toRef}</span>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-[13px] text-ink/50">No lineage edges.</p>
        )}
      </div>
    </div>
  );
}

export function DocumentsGroup() {
  const [rows, setRows] = useState<DocumentRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [statuses, setStatuses] = useState<string[]>([]);
  const [active, setActive] = useState<DocumentRow | null>(null);

  function reload() {
    setLoading(true);
    setError(null);
    api.documents({ limit: 300 }).then(
      (r) => { setRows(r.documents); setLoading(false); },
      (e) => { setError(String(e?.message ?? e)); setLoading(false); },
    );
  }

  useEffect(() => { reload(); }, []);
  useEffect(() => {
    api.tags().then((r) => setStatuses(r.statuses)).catch(() => setStatuses([]));
  }, []);

  const columns: Column<DocumentRow>[] = [
    {
      key: "title", header: "Title", sortable: true, sort: (r) => docTitle(r).toLowerCase(),
      render: (r) => {
        const sub = docSubRef(r);
        return (
          <div className="min-w-0">
            <div className="text-[13px] font-medium text-ink truncate max-w-[360px]">{docTitle(r)}</div>
            {sub && <div className="font-term text-[11px] text-ink/45 truncate max-w-[360px]">{sub}</div>}
          </div>
        );
      },
    },
    { key: "provider", header: "Source", sortable: true, sort: (r) => r.provider, render: (r) => <Badge label={r.provider} tone="neutral" /> },
    { key: "kind", header: "Kind", sortable: true, sort: (r) => r.kind, render: (r) => <span className="text-[12.5px] text-ink/75">{r.kind}</span> },
    { key: "updated", header: "Updated", sortable: true, sort: (r) => r.updated_at ?? "", render: (r) => <span className="font-term text-[12px] text-ink/60 whitespace-nowrap">{relTime(r.updated_at)}</span> },
    { key: "tag", header: "Tag", sortable: true, sort: (r) => r.tag ?? "", render: (r) => (r.tag ? <Badge label={r.tag} tone={tagTone(r.tag)} /> : <span className="text-ink/35">—</span>) },
  ];

  return (
    <Page title="Documents" subtitle="Every indexed document in your knowledge base." kicker="content">
      {loading ? (
        <div className={`${card} mt-5 grid place-items-center py-20 text-ink/50`}>
          <Loader2 size={22} className="animate-spin" />
        </div>
      ) : error ? (
        <div className={`${card} mt-5 p-6`}>
          <p className="text-[13px] text-espelette">Failed to load documents. {error}</p>
          <button className={`${btn} mt-3`} onClick={reload}>Retry</button>
        </div>
      ) : (
        <DataTable<DocumentRow>
          title="Documents"
          count={rows.length}
          rows={rows}
          columns={columns}
          rowKey={(r) => r.doc_id}
          search={(r) => `${r.title ?? ""} ${r.path ?? ""} ${r.canonical_ref}`}
          searchPlaceholder="Search documents…"
          facet={{ label: "sources", get: (r) => r.provider }}
          onRowClick={(r) => setActive(r)}
          pageSize={12}
          empty="Nothing indexed yet — connect a source and sync."
        />
      )}

      <Drawer
        open={!!active}
        onClose={() => setActive(null)}
        title={active ? docTitle(active) : ""}
        subtitle={active?.provider}
        icon={<FileText size={18} className="text-biscay-2" />}
      >
        {active && <DocDetail row={active} statuses={statuses} onChanged={reload} />}
      </Drawer>
    </Page>
  );
}
