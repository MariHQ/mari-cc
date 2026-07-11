import { useEffect, useState, type ReactNode } from "react";
import {
  ResponsiveContainer,
  BarChart,
  Bar,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
  LabelList,
} from "recharts";
import { api, type Overview, type SyncEvent } from "@saas/lib/client";
import { Page, Table, Badge, card } from "../console-ui";

/* Chart palette — muted, professional. Kept in sync with the blueprint tokens. */
const C = {
  biscay: "#1C3F60",
  biscay2: "#1E6FA8",
  moss: "#2C6E49",
  clay: "#A05E1C",
  espelette: "#B23A1E",
  grid: "rgb(0 0 0 / 0.08)",
  tick: "#6b7280",
};

/* Map a tag status onto a chart color. Unknown statuses fall back to biscay-2. */
function tagColor(status: string): string {
  switch (status.toLowerCase()) {
    case "canonical":
      return C.moss;
    case "draft":
      return "#6b7280";
    case "stale":
      return C.clay;
    case "deprecated":
      return C.espelette;
    default:
      return C.biscay2;
  }
}

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

function syncTone(status: string): string {
  const s = status.toLowerCase();
  if (s === "ok" || s === "success") return "ok";
  if (s === "error") return "blocked";
  return "attention";
}

function Kpi({ label, value }: { label: string; value: number }) {
  return (
    <div className={`${card} px-4 py-3.5`}>
      <div className="font-term text-[10.5px] uppercase tracking-[0.12em] text-ink/55">{label}</div>
      <div className="mt-1.5 font-term text-[30px] leading-none font-semibold tracking-[-0.02em] text-ink tabular-nums">
        {value.toLocaleString()}
      </div>
    </div>
  );
}

/* A `card` with a title header + optional right-side note. */
function ChartCard({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  return (
    <div className={`${card} p-4`}>
      <div className="flex items-center justify-between gap-2">
        <h4 className="text-[15px] font-semibold text-ink">{title}</h4>
        {note && <div className="font-term text-[11px] text-ink/50">{note}</div>}
      </div>
      <div className="mt-3">{children}</div>
    </div>
  );
}

/* Muted placeholder box sized to match the charts. */
function Placeholder({ label, height = 240 }: { label: string; height?: number }) {
  return (
    <div
      className="grid place-items-center rounded-[4px] border border-dashed border-ink/15 bg-flysch/40 text-[12.5px] text-ink/45"
      style={{ height }}
    >
      {label}
    </div>
  );
}

/* Simple, on-brand tooltip. */
function ChartTooltip({ active, payload, label, unit = "" }: any) {
  if (!active || !payload || payload.length === 0) return null;
  const p = payload[0];
  const name = label ?? p?.payload?.name ?? p?.name ?? "";
  const value = typeof p?.value === "number" ? p.value.toLocaleString() : p?.value;
  return (
    <div className={`${card} px-2.5 py-1.5 shadow-sm`}>
      <div className="font-term text-[11px] text-ink/60">{name}</div>
      <div className="font-term text-[12.5px] font-semibold text-ink tabular-nums">
        {value}
        {unit}
      </div>
    </div>
  );
}

export function OverviewGroup() {
  const [data, setData] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .overview()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  return (
    <Page title="Overview" subtitle="Your knowledge base at a glance." kicker="mari">
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load overview</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && <OverviewBody data={data} />}
    </Page>
  );
}

function OverviewBody({ data }: { data: Overview }) {
  const { kpis, tagCounts, freshness, perSource, recentSyncs } = data;

  const totalFresh = freshness.fresh + freshness.stale;
  const freshPct = totalFresh > 0 ? Math.round((freshness.fresh / totalFresh) * 100) : 0;
  const emptyInstall = kpis.documents === 0;

  // Documents by source — sorted desc.
  const sources = [...perSource]
    .filter((s) => s.documents > 0 || perSource.length <= 8)
    .sort((a, b) => b.documents - a.documents)
    .map((s) => ({ name: s.provider, documents: s.documents }));

  // Tag distribution.
  const tags = [...tagCounts]
    .filter((t) => t.n > 0)
    .sort((a, b) => b.n - a.n)
    .map((t) => ({ name: t.status, value: t.n, fill: tagColor(t.status) }));
  const tagTotal = tags.reduce((sum, t) => sum + t.value, 0);

  // Recent sync activity — chronological (oldest → newest), most recent 8.
  const syncActivity = [...recentSyncs]
    .slice(0, 8)
    .reverse()
    .map((ev, i) => ({
      name: relTime(ev.started_at),
      changed: ev.docs_changed ?? 0,
      key: `${ev.source_id}-${ev.started_at ?? i}`,
    }));
  const hasSyncActivity = syncActivity.some((s) => s.changed > 0);

  // Give horizontal bars room to breathe: ~34px per row, min 240.
  const barsHeight = Math.max(240, sources.length * 34 + 24);

  return (
    <>
      {/* KPI grid */}
      <div className="mt-5 grid grid-cols-2 lg:grid-cols-4 gap-3">
        <Kpi label="Documents" value={kpis.documents} />
        <Kpi label="Connectors connected" value={kpis.sourcesConnected} />
        <Kpi label="Proposed lineage" value={kpis.proposedLineage} />
        <Kpi label="Tags" value={kpis.tags} />
      </div>

      {emptyInstall && (
        <div className={`${card} mt-4 p-4 border-biscay-2/25 bg-biscay-2/[0.04]`}>
          <div className="text-[13px] font-medium text-ink">Nothing indexed yet</div>
          <div className="mt-1 text-[12.5px] text-ink/65">
            Connect a source and run a sync from <span className="font-term">Sources</span> to populate your
            knowledge base. Charts fill in once documents land.
          </div>
        </div>
      )}

      {/* Chart row: Documents by source + Tag distribution */}
      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <ChartCard title="Documents by source" note={sources.length ? `${sources.length} sources` : undefined}>
          {sources.length === 0 ? (
            <Placeholder label="No documents yet" />
          ) : (
            <ResponsiveContainer width="100%" height={Math.min(barsHeight, 340)}>
              <BarChart
                layout="vertical"
                data={sources}
                margin={{ top: 4, right: 40, bottom: 4, left: 8 }}
                barCategoryGap={10}
              >
                <CartesianGrid horizontal={false} stroke={C.grid} />
                <XAxis
                  type="number"
                  allowDecimals={false}
                  tick={{ fontSize: 11, fill: C.tick, fontFamily: "var(--font-term, monospace)" }}
                  axisLine={{ stroke: C.grid }}
                  tickLine={false}
                />
                <YAxis
                  type="category"
                  dataKey="name"
                  width={110}
                  tick={{ fontSize: 11, fill: C.tick, fontFamily: "var(--font-term, monospace)" }}
                  axisLine={false}
                  tickLine={false}
                />
                <Tooltip cursor={{ fill: "rgb(0 0 0 / 0.04)" }} content={<ChartTooltip unit=" docs" />} />
                <Bar dataKey="documents" fill={C.biscay2} radius={[0, 3, 3, 0]} maxBarSize={22}>
                  <LabelList
                    dataKey="documents"
                    position="right"
                    formatter={(v: number) => v.toLocaleString()}
                    style={{ fontSize: 11, fill: C.tick, fontFamily: "var(--font-term, monospace)" }}
                  />
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          )}
        </ChartCard>

        <ChartCard title="Tag distribution" note={tagTotal ? `${tagTotal.toLocaleString()} tagged` : undefined}>
          {tags.length === 0 ? (
            <Placeholder label="No tags applied yet" />
          ) : (
            <div className="flex flex-col sm:flex-row items-center gap-4">
              <div className="w-full sm:w-1/2 shrink-0">
                <ResponsiveContainer width="100%" height={220}>
                  <PieChart>
                    <Pie
                      data={tags}
                      dataKey="value"
                      nameKey="name"
                      cx="50%"
                      cy="50%"
                      innerRadius={54}
                      outerRadius={84}
                      paddingAngle={2}
                      stroke="var(--color-paper, #fff)"
                      strokeWidth={1.5}
                    >
                      {tags.map((t) => (
                        <Cell key={t.name} fill={t.fill} />
                      ))}
                    </Pie>
                    <Tooltip content={<ChartTooltip />} />
                  </PieChart>
                </ResponsiveContainer>
              </div>
              <div className="w-full sm:w-1/2 grid grid-cols-1 gap-1.5">
                {tags.map((t) => (
                  <div key={t.name} className="flex items-center gap-2 font-term text-[11.5px]">
                    <span
                      className="inline-block w-2.5 h-2.5 rounded-[2px] shrink-0"
                      style={{ background: t.fill }}
                      aria-hidden
                    />
                    <span className="text-ink/75 truncate">{t.name}</span>
                    <span className="ml-auto text-ink/55 tabular-nums">
                      {t.value.toLocaleString()}
                      <span className="text-ink/35">
                        {" "}
                        · {tagTotal ? Math.round((t.value / tagTotal) * 100) : 0}%
                      </span>
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </ChartCard>
      </div>

      {/* Freshness + (optional) recent sync activity */}
      <div className="mt-5 grid gap-5 lg:grid-cols-2">
        <div className={`${card} p-4`}>
          <h4 className="text-[15px] font-semibold text-ink">Freshness</h4>
          {totalFresh === 0 ? (
            <p className="mt-3 text-[12.5px] text-ink/55">No documents to measure yet.</p>
          ) : (
            <>
              <div className="mt-3 flex h-3 w-full overflow-hidden rounded-[3px] bg-flysch border border-ink/10">
                <div className="h-full bg-moss" style={{ width: `${freshPct}%` }} aria-hidden />
                <div className="h-full bg-clay flex-1" aria-hidden />
              </div>
              <div className="mt-2.5 flex items-center gap-4 font-term text-[11.5px]">
                <span className="flex items-center gap-1.5 text-ink/70">
                  <span className="inline-block w-2 h-2 rounded-[2px] bg-moss" />{" "}
                  {freshness.fresh.toLocaleString()} fresh
                </span>
                <span className="flex items-center gap-1.5 text-ink/70">
                  <span className="inline-block w-2 h-2 rounded-[2px] bg-clay" />{" "}
                  {freshness.stale.toLocaleString()} stale
                </span>
              </div>
              <p className="mt-2 text-[12px] text-ink/55">
                {freshPct}% fresh — documents untouched past the stale window are marked stale.
              </p>
            </>
          )}
        </div>

        <ChartCard title="Recent sync activity" note="docs changed">
          {hasSyncActivity ? (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={syncActivity} margin={{ top: 8, right: 8, bottom: 4, left: -12 }} barCategoryGap={8}>
                <CartesianGrid vertical={false} stroke={C.grid} />
                <XAxis
                  dataKey="name"
                  tick={{ fontSize: 10, fill: C.tick, fontFamily: "var(--font-term, monospace)" }}
                  axisLine={{ stroke: C.grid }}
                  tickLine={false}
                  interval={0}
                />
                <YAxis
                  allowDecimals={false}
                  tick={{ fontSize: 11, fill: C.tick, fontFamily: "var(--font-term, monospace)" }}
                  axisLine={false}
                  tickLine={false}
                  width={40}
                />
                <Tooltip cursor={{ fill: "rgb(0 0 0 / 0.04)" }} content={<ChartTooltip unit=" changed" />} />
                <Bar dataKey="changed" fill={C.biscay} radius={[3, 3, 0, 0]} maxBarSize={34} />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <Placeholder label="No sync activity yet" height={220} />
          )}
        </ChartCard>
      </div>

      {/* Recent syncs table */}
      <Table
        title="Recent syncs"
        count={recentSyncs.length}
        head={["Source", "Status", "Started", "Docs", "Detail"]}
        minW={720}
      >
        {recentSyncs.length === 0 ? (
          <tr>
            <td colSpan={5} className="px-4 py-10 text-center text-[13px] text-ink/55">
              No syncs yet — run a sync from Sources.
            </td>
          </tr>
        ) : (
          recentSyncs.map((ev: SyncEvent, i) => (
            <tr key={`${ev.source_id}-${ev.started_at ?? i}`} className="border-b border-ink/10 last:border-0">
              <td className="px-4 py-3 font-term text-[12.5px] text-ink/85">{ev.source_id}</td>
              <td className="px-4 py-3">
                <Badge label={ev.status} tone={syncTone(ev.status)} />
              </td>
              <td
                className="px-4 py-3 font-term text-[12px] text-ink/65 whitespace-nowrap"
                title={ev.started_at ?? undefined}
              >
                {relTime(ev.started_at)}
              </td>
              <td className="px-4 py-3 font-term text-[12px] text-ink/70 whitespace-nowrap tabular-nums">
                {ev.docs_seen} seen · {ev.docs_changed} changed
              </td>
              <td className="px-4 py-3 text-[12px]">
                {ev.error ? (
                  <span className="text-espelette" title={ev.error}>
                    {ev.error}
                  </span>
                ) : (
                  <span className="text-ink/35">—</span>
                )}
              </td>
            </tr>
          ))
        )}
      </Table>
    </>
  );
}
