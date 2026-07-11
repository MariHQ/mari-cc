import { useEffect, useMemo, useState } from "react";
import { RefreshCw, Plug, X, Plus } from "lucide-react";
import { api, type Source } from "@saas/lib/client";
import {
  Page,
  DataTable,
  Drawer,
  Field,
  Badge,
  btn,
  btnPrimary,
  focusRing,
  type Column,
} from "../console-ui";
import { toast } from "../feedback";

/* ── relative-time helper for ISO strings (null-guarded) ─────────────────── */
function relTime(iso: string | null): string {
  if (!iso) return "—";
  const t = new Date(iso).getTime();
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

function trackedCount(s: Source): number {
  return s.tracked.reduce((n, g) => n + g.refs.length, 0);
}

/* Compact stringify of a config value for read-only display. */
function fmtConfig(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

/* ── Tracked-reference editor: chips + add input, per tracked group ──────── */
function TrackedGroup({
  source,
  group,
  onChanged,
}: {
  source: string;
  group: { key: string; refs: string[] };
  onChanged: () => void;
}) {
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);

  const remove = async (ref: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await api.track(source, ref, "remove", group.key);
      toast(`Removed ${ref}`, "success");
      onChanged();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to remove reference", "error");
    } finally {
      setBusy(false);
    }
  };

  const add = async () => {
    const ref = input.trim();
    if (!ref || busy) return;
    setBusy(true);
    try {
      await api.track(source, ref, "add", group.key);
      toast(`Tracking ${ref}`, "success");
      setInput("");
      onChanged();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to add reference", "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="py-2.5 border-b border-ink/10 last:border-0">
      <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55">{group.key}</div>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {group.refs.length === 0 && <span className="text-[12.5px] text-ink/45">No references tracked.</span>}
        {group.refs.map((ref) => (
          <span
            key={ref}
            className="inline-flex items-center gap-1 rounded-[3px] border border-ink/20 bg-flysch pl-2 pr-1 py-[3px] font-term text-[11.5px] text-ink/80"
          >
            <span className="truncate max-w-[220px]">{ref}</span>
            <button
              onClick={() => remove(ref)}
              disabled={busy}
              aria-label={`Stop tracking ${ref}`}
              className={`grid place-items-center w-4 h-4 rounded-[2px] text-ink/45 hover:text-espelette hover:bg-espelette/10 disabled:opacity-40 ${focusRing}`}
            >
              <X size={12} />
            </button>
          </span>
        ))}
      </div>
      <div className="mt-2 flex items-center gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") { e.preventDefault(); add(); }
          }}
          placeholder="Add reference…"
          className="flex-1 h-8 px-2.5 rounded-[4px] border border-ink/20 bg-paper text-[12.5px] text-ink placeholder:text-ink/45 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40"
        />
        <button onClick={add} disabled={busy || !input.trim()} className={`${btn} h-8 disabled:opacity-40`}>
          <Plus size={13} /> Track
        </button>
      </div>
    </div>
  );
}

export function SourcesGroup() {
  const [sources, setSources] = useState<Source[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [syncingAll, setSyncingAll] = useState(false);
  const [syncingOne, setSyncingOne] = useState(false);

  const reload = async () => {
    try {
      setError(null);
      const r = await api.sources();
      setSources(r.sources);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load sources");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    reload();
  }, []);

  // Re-derive the selected source from fresh data so the drawer stays in sync.
  const selected = useMemo(
    () => (selectedId ? sources.find((s) => s.source === selectedId) ?? null : null),
    [sources, selectedId],
  );

  const syncAll = async () => {
    setSyncingAll(true);
    toast("Sync started for all sources…");
    try {
      await api.sync();
      toast("Sync complete", "success");
      await reload();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Sync failed", "error");
    } finally {
      setSyncingAll(false);
    }
  };

  const syncOne = async (source: string) => {
    setSyncingOne(true);
    toast("Sync started — this may take a while…");
    try {
      await api.sync(source);
      toast("Sync complete", "success");
      await reload();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Sync failed", "error");
    } finally {
      setSyncingOne(false);
    }
  };

  const columns: Column<Source>[] = [
    {
      key: "source",
      header: "Source",
      sortable: true,
      sort: (s) => s.source,
      render: (s) => (
        <div className="min-w-0">
          <div className="font-medium text-ink truncate">{s.source}</div>
          <div className="font-term text-[11px] text-ink/55 truncate">
            {s.credentialFree ? "local / no credential" : s.authProvider ?? "—"}
          </div>
        </div>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: (s) => (
        <div className="flex flex-wrap items-center gap-1.5">
          {s.connected ? (
            <Badge label="connected" tone="ok" />
          ) : s.credentialFree ? (
            <Badge label="ready" tone="info" />
          ) : (
            <Badge label="not connected" tone="attention" />
          )}
          {s.lastError && <Badge label="error" tone="blocked" />}
        </div>
      ),
    },
    {
      key: "scope",
      header: "Scope",
      sortable: true,
      sort: (s) => s.scope,
      render: (s) => <Badge label={s.scope} tone="neutral" />,
    },
    {
      key: "indexed",
      header: "Indexed",
      align: "right",
      sortable: true,
      sort: (s) => s.indexedDocuments,
      cell: "font-term text-[12.5px] text-ink/80",
      render: (s) => s.indexedDocuments.toLocaleString(),
    },
    {
      key: "tracked",
      header: "Tracked",
      align: "right",
      sortable: true,
      sort: (s) => trackedCount(s),
      cell: "font-term text-[12.5px] text-ink/80",
      render: (s) => trackedCount(s),
    },
    {
      key: "lastSync",
      header: "Last sync",
      align: "right",
      sortable: true,
      sort: (s) => (s.lastSyncAt ? new Date(s.lastSyncAt).getTime() : 0),
      cell: "font-term text-[12px] text-ink/70",
      render: (s) => relTime(s.lastSyncAt),
    },
  ];

  return (
    <Page
      title="Sources"
      subtitle="Connectors, what they track, and their sync status."
      kicker="content"
      actions={
        <button onClick={syncAll} disabled={syncingAll} className={`${btnPrimary} disabled:opacity-60`}>
          <RefreshCw size={14} className={syncingAll ? "animate-spin" : ""} />
          {syncingAll ? "Syncing…" : "Sync all"}
        </button>
      }
    >
      {loading ? (
        <div className="mt-5 grid place-items-center py-20 text-[13px] text-ink/55 font-term">Loading sources…</div>
      ) : error ? (
        <div className="mt-5 grid place-items-center gap-3 py-16 text-center">
          <p className="text-[13px] text-espelette">{error}</p>
          <button onClick={reload} className={btn}>Retry</button>
        </div>
      ) : (
        <DataTable<Source>
          title="Connectors"
          count={sources.length}
          rows={sources}
          columns={columns}
          rowKey={(s) => s.source}
          search={(s) => s.source}
          searchPlaceholder="Search sources…"
          facet={{ label: "scopes", get: (s) => s.scope }}
          onRowClick={(s) => setSelectedId(s.source)}
          pageSize={10}
          empty="No sources configured yet."
        />
      )}

      <Drawer
        open={!!selected}
        onClose={() => setSelectedId(null)}
        title={selected?.source ?? ""}
        subtitle={selected ? (selected.credentialFree ? "local / no credential" : selected.authProvider ?? "no auth") : undefined}
        icon={<Plug size={18} className="text-biscay-2" />}
        footer={
          selected && (
            <button onClick={() => syncOne(selected.source)} disabled={syncingOne} className={`${btnPrimary} w-full justify-center disabled:opacity-60`}>
              <RefreshCw size={14} className={syncingOne ? "animate-spin" : ""} />
              {syncingOne ? "Syncing…" : "Sync now"}
            </button>
          )
        }
      >
        {selected && (
          <div>
            <Field label="Status">
              <div className="flex flex-wrap items-center gap-1.5">
                {selected.connected ? (
                  <Badge label="connected" tone="ok" />
                ) : selected.credentialFree ? (
                  <Badge label="ready" tone="info" />
                ) : (
                  <Badge label="not connected" tone="attention" />
                )}
                {selected.lastError && <Badge label="error" tone="blocked" />}
              </div>
            </Field>
            <Field label="Auth provider">
              {selected.authProvider ? (
                <span className="font-term text-[12.5px]">{selected.authProvider}</span>
              ) : (
                <span className="text-ink/60">none — local source</span>
              )}
            </Field>
            <Field label="Scope">
              <Badge label={selected.scope} tone="neutral" />
            </Field>
            <Field label="Indexed documents">
              <span className="font-term text-[12.5px]">{selected.indexedDocuments.toLocaleString()}</span>
            </Field>
            <Field label="Last sync">{relTime(selected.lastSyncAt)}</Field>
            {selected.lastError && (
              <Field label="Last error">
                <span className="text-espelette break-words">{selected.lastError}</span>
              </Field>
            )}

            <div className="mt-5">
              <div className="text-[13px] font-semibold text-ink mb-1">Tracked references</div>
              <p className="text-[12px] text-ink/55 mb-1">What this connector pulls into the index.</p>
              {selected.tracked.length === 0 ? (
                <p className="py-2 text-[12.5px] text-ink/50">This source has no tracked-reference lists.</p>
              ) : (
                selected.tracked.map((group) => (
                  <TrackedGroup key={group.key} source={selected.source} group={group} onChanged={reload} />
                ))
              )}
            </div>

            <div className="mt-5">
              <div className="text-[13px] font-semibold text-ink mb-1">Sync now</div>
              <p className="text-[12px] text-ink/55">
                Re-index this source. This may take a while for large connectors.
              </p>
            </div>

            <div className="mt-5">
              <div className="text-[13px] font-semibold text-ink mb-1">Configuration</div>
              <p className="text-[12px] text-ink/55 mb-1">Edit these on the Config page.</p>
              {Object.keys(selected.config).length === 0 ? (
                <p className="py-2 text-[12.5px] text-ink/50">No configuration set.</p>
              ) : (
                Object.entries(selected.config).map(([k, v]) => (
                  <Field key={k} label={k}>
                    <span className="font-term text-[12px] break-words">{fmtConfig(v)}</span>
                  </Field>
                ))
              )}
            </div>
          </div>
        )}
      </Drawer>
    </Page>
  );
}
