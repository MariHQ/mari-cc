import { useEffect, useState } from "react";
import { Plus, Tag as TagIcon, X } from "lucide-react";
import { api, type TagRow } from "@saas/lib/client";
import { Page, DataTable, Drawer, Badge, btn, btnPrimary, btnDanger, card, focusRing, type Column } from "../console-ui";
import { toast } from "../feedback";

/* Format an ISO timestamp as a compact relative string ("3d ago"). */
function relTime(iso: string | null): string {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const sec = Math.round((Date.now() - t) / 1000);
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

/* Tag status → Badge tone. */
function tagTone(status: string): string {
  switch (status.toLowerCase()) {
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

const inputCls =
  `w-full h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] text-ink placeholder:text-ink/40 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`;
const labelCls = "font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55";

export function TagsGroup() {
  const [rows, setRows] = useState<TagRow[]>([]);
  const [statuses, setStatuses] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [addOpen, setAddOpen] = useState(false);
  const [ref, setRef] = useState("");
  const [status, setStatus] = useState("");
  const [note, setNote] = useState("");
  const [supersededBy, setSupersededBy] = useState("");
  const [saving, setSaving] = useState(false);

  const [newStatus, setNewStatus] = useState("");
  const [vocabBusy, setVocabBusy] = useState(false);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .tags()
      .then((d) => { setRows(d.tags); setStatuses(d.statuses); })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  function openAdd() {
    setRef("");
    setStatus(statuses[0] ?? "");
    setNote("");
    setSupersededBy("");
    setAddOpen(true);
  }

  async function commitStatuses(next: string[], msg: string) {
    setVocabBusy(true);
    try {
      await api.setTagStatuses(next);
      toast(msg, "success");
      reload();
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setVocabBusy(false);
    }
  }

  async function removeStatus(s: string) {
    if (statuses.length <= 1) {
      toast("Keep at least one status in the vocabulary", "error");
      return;
    }
    await commitStatuses(statuses.filter((x) => x !== s), `Removed “${s}”`);
  }

  async function addStatus() {
    const v = newStatus.trim().toLowerCase();
    if (!v) return;
    if (statuses.includes(v)) {
      toast("That status already exists", "error");
      return;
    }
    setNewStatus("");
    await commitStatuses([...statuses, v], `Added “${v}”`);
  }

  async function remove(r: TagRow, e: React.MouseEvent) {
    e.stopPropagation();
    try {
      await api.removeTag(r.ref);
      toast("Tag removed", "success");
      reload();
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    }
  }

  async function apply() {
    if (!ref.trim() || !status.trim()) {
      toast("Reference and status are required", "error");
      return;
    }
    setSaving(true);
    try {
      await api.applyTag(ref.trim(), status, note.trim() || undefined, supersededBy.trim() || undefined);
      toast("Tagged", "success");
      setAddOpen(false);
      reload();
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setSaving(false);
    }
  }

  const columns: Column<TagRow>[] = [
    {
      key: "ref",
      header: "Reference",
      sortable: true,
      sort: (r) => r.ref,
      render: (r) => (
        <div className="min-w-0">
          <div className="font-term text-[12.5px] text-ink/90 truncate">{r.ref}</div>
          {r.title && <div className="text-[11.5px] text-ink/50 truncate">{r.title}</div>}
        </div>
      ),
    },
    {
      key: "status",
      header: "Status",
      sortable: true,
      sort: (r) => r.status,
      render: (r) => <Badge label={r.status} tone={tagTone(r.status)} />,
    },
    {
      key: "note",
      header: "Note",
      render: (r) => r.note ? <span className="text-[12.5px] text-ink/70">{r.note}</span> : <span className="text-ink/30">—</span>,
    },
    {
      key: "by",
      header: "By",
      render: (r) => <span className="font-term text-[12px] text-ink/70">{r.by || "—"}</span>,
    },
    {
      key: "at",
      header: "At",
      sortable: true,
      sort: (r) => Date.parse(r.at) || 0,
      render: (r) => <span className="font-term text-[12px] text-ink/60 whitespace-nowrap" title={r.at}>{relTime(r.at)}</span>,
    },
    {
      key: "actions",
      header: "",
      align: "right",
      render: (r) => (
        <button onClick={(e) => remove(r, e)} className={`${btnDanger} h-7 px-2 text-[12px]`}>Remove</button>
      ),
    },
  ];

  return (
    <Page
      title="Tags"
      subtitle="Curation tags applied to documents, and the status vocabulary."
      kicker="curation"
      actions={
        <button onClick={openAdd} className={btnPrimary}>
          <Plus size={15} /> Add tag
        </button>
      }
    >
      {loading && rows.length === 0 && !error && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className="mt-5 rounded-md border border-espelette/30 bg-espelette/[0.05] p-4">
          <div className="text-[13px] font-medium text-espelette">Couldn’t load tags</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">Retry</button>
        </div>
      )}

      {!error && !(loading && rows.length === 0) && (
        <div className="mt-5 space-y-5">
          {/* Status vocabulary manager */}
          <div className={`${card} p-4`}>
            <div className="flex items-baseline justify-between gap-3">
              <h2 className="text-[13px] font-semibold text-ink">Statuses</h2>
              <span className="font-term text-[11px] text-ink/45">{statuses.length}</span>
            </div>

            <div className="mt-3 flex flex-wrap gap-2">
              {statuses.length === 0 && (
                <span className="text-[12.5px] text-ink/40">No statuses defined yet.</span>
              )}
              {statuses.map((s) => (
                <span
                  key={s}
                  className="inline-flex items-center gap-1.5 rounded-[4px] border border-ink/10 bg-flysch/40 py-1 pl-1.5 pr-1"
                >
                  <Badge label={s} tone={tagTone(s)} />
                  <button
                    type="button"
                    onClick={() => removeStatus(s)}
                    disabled={vocabBusy || statuses.length <= 1}
                    title={statuses.length <= 1 ? "Keep at least one status" : `Remove “${s}”`}
                    className={`grid h-5 w-5 place-items-center rounded-[3px] text-ink/40 hover:bg-espelette/10 hover:text-espelette disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-ink/40 ${focusRing}`}
                  >
                    <X size={13} />
                  </button>
                </span>
              ))}
            </div>

            <div className="mt-3 flex items-center gap-2">
              <input
                value={newStatus}
                onChange={(e) => setNewStatus(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void addStatus(); } }}
                placeholder="new-status"
                spellCheck={false}
                className={`max-w-[240px] font-term ${inputCls}`}
              />
              <button
                type="button"
                onClick={() => void addStatus()}
                disabled={vocabBusy || !newStatus.trim()}
                className={`${btn} disabled:opacity-50`}
              >
                <Plus size={14} /> Add
              </button>
            </div>

            <p className="mt-2.5 text-[11.5px] leading-relaxed text-ink/45">
              The vocabulary <code className="font-term text-ink/60">mari tag</code> accepts.
              Removing a status doesn’t untag existing docs.
            </p>
          </div>

          {/* Main surface: tagged documents */}
          <DataTable<TagRow>
            title="Tagged documents"
            count={rows.length}
            rows={rows}
            columns={columns}
            rowKey={(r) => r.ref}
            search={(r) => `${r.ref} ${r.title ?? ""}`}
            searchPlaceholder="Search reference or title…"
            facet={{ label: "statuses", get: (r) => r.status }}
            empty="No tags yet. Tag a document to mark it canonical, stale, deprecated, or draft."
          />
        </div>
      )}

      <Drawer
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title="Add tag"
        subtitle="curation"
        icon={<TagIcon size={18} className="text-biscay-2" />}
        footer={
          <button onClick={apply} disabled={saving} className={`${btnPrimary} disabled:opacity-50`}>
            {saving ? "Applying…" : "Apply tag"}
          </button>
        }
      >
        <div className="space-y-4">
          <label className="block">
            <div className={labelCls}>Reference</div>
            <input
              value={ref}
              onChange={(e) => setRef(e.target.value)}
              placeholder="path or ref, e.g. docs/setup.md"
              className={`mt-1.5 font-term ${inputCls}`}
            />
          </label>
          <label className="block">
            <div className={labelCls}>Status</div>
            <select
              value={status}
              onChange={(e) => setStatus(e.target.value)}
              className={`mt-1.5 ${inputCls}`}
            >
              <option value="" disabled>Select a status…</option>
              {statuses.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
          </label>
          <label className="block">
            <div className={labelCls}>Note <span className="normal-case tracking-normal text-ink/40">(optional)</span></div>
            <input
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Why this tag?"
              className={`mt-1.5 ${inputCls}`}
            />
          </label>
          <label className="block">
            <div className={labelCls}>Superseded by <span className="normal-case tracking-normal text-ink/40">(optional)</span></div>
            <input
              value={supersededBy}
              onChange={(e) => setSupersededBy(e.target.value)}
              placeholder="Replacement ref"
              className={`mt-1.5 font-term ${inputCls}`}
            />
          </label>
        </div>
      </Drawer>
    </Page>
  );
}
