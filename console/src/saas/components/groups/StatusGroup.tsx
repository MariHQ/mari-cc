import { useEffect, useState, type ReactNode } from "react";
import { RefreshCw } from "lucide-react";
import { api, type Status } from "@saas/lib/client";
import { Page, Field, Badge, card, btnPrimary } from "../console-ui";
import { toast } from "../feedback";

/* Compact relative time ("3d ago") with null guard. */
function relTime(iso: string | null): string {
  if (!iso) return "never";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "never";
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

function absTime(iso: string | null): string {
  if (!iso) return "";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  return new Date(t).toLocaleString();
}

const mono = "font-term text-[12.5px] text-ink/85 break-all";

function StatCard({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className={`${card} px-4 py-2`}>
      <h4 className="text-[15px] font-semibold text-ink pt-2 pb-1">{title}</h4>
      {children}
    </div>
  );
}

function CountStat({ label, value }: { label: string; value: number }) {
  return (
    <div className="px-3 py-2.5 rounded-[4px] border border-ink/10 bg-flysch/40">
      <div className="font-term text-[10px] uppercase tracking-[0.1em] text-ink/55">{label}</div>
      <div className="mt-1 font-term text-[22px] leading-none font-semibold text-ink tabular-nums">
        {value.toLocaleString()}
      </div>
    </div>
  );
}

export function StatusGroup() {
  const [data, setData] = useState<Status | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .status()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  async function syncAll() {
    if (syncing) return;
    setSyncing(true);
    try {
      await api.sync();
      toast("Sync complete", "success");
      reload();
    } catch (e: unknown) {
      toast(e instanceof Error ? e.message : String(e), "error");
    } finally {
      setSyncing(false);
    }
  }

  const syncButton = (
    <button
      onClick={syncAll}
      disabled={syncing}
      className={`${btnPrimary} disabled:opacity-60 disabled:cursor-not-allowed`}
      title="Syncs every connected source. This can take a while."
    >
      <RefreshCw size={14} className={syncing ? "animate-spin" : ""} />
      {syncing ? "Syncing…" : "Sync all"}
    </button>
  );

  return (
    <Page
      title="Status"
      subtitle="Workspace, embedding model, catalog, and cloud."
      kicker="mari"
      actions={syncButton}
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load status</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && <StatusBody data={data} />}
    </Page>
  );
}

function StatusBody({ data }: { data: Status }) {
  const c = data.counts;
  return (
    <>
      {data.counts.documents === 0 && (
        <div className={`${card} mt-5 p-4 border-biscay-2/25 bg-biscay-2/[0.04]`}>
          <div className="text-[13px] font-medium text-ink">No documents indexed yet.</div>
          <div className="mt-1 text-[12.5px] text-ink/65">Connect a source and sync.</div>
        </div>
      )}

      <div className="mt-5 grid grid-cols-1 lg:grid-cols-2 gap-3">
        <StatCard title="Workspace">
          <Field label="Workspace">
            <span className={mono}>{data.workspace}</span>
          </Field>
          <Field label="Catalog">
            <span className={mono}>{data.catalog}</span>
          </Field>
          <Field label="Embedding model">
            <span className="font-term text-[12.5px] text-ink/85">{data.embeddingModel}</span>
          </Field>
        </StatCard>

        <StatCard title="Sync & cloud">
          <Field label="Last sync">
            {data.lastSync ? (
              <span title={absTime(data.lastSync)}>
                {relTime(data.lastSync)}
                <span className="text-ink/45"> · {absTime(data.lastSync)}</span>
              </span>
            ) : (
              <span className="text-ink/55">never</span>
            )}
          </Field>
          <Field label="Stale threshold">
            <span className="font-term text-[12.5px] text-ink/85 tabular-nums">{data.staleDays}</span>
            <span className="text-ink/55"> days</span>
          </Field>
          <Field label="Cloud">
            {data.cloudEnabled ? <Badge label="enabled" tone="ok" /> : <Badge label="disabled" tone="neutral" />}
          </Field>
        </StatCard>
      </div>

      <StatCard title="Counts">
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5 pb-3">
          <CountStat label="Documents" value={c.documents} />
          <CountStat label="Chunks" value={c.chunks} />
          <CountStat label="Tags" value={c.tags} />
          <CountStat label="Lineage edges" value={c.lineageEdges} />
        </div>
      </StatCard>

      <p className="mt-3 text-[12px] text-ink/50">
        “Sync all” refreshes every connected source and can take seconds to minutes.
      </p>
    </>
  );
}
