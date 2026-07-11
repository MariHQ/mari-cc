import { useEffect, useMemo, useState } from "react";
import { Plus, GitBranch, Check, X as XIcon, Search, RotateCcw, Crosshair } from "lucide-react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  MarkerType,
  type Node,
  type Edge as RFEdge,
  type NodeMouseHandler,
  type EdgeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import dagre from "@dagrejs/dagre";
import { api, type LineageEdge } from "@saas/lib/client";
import { Page, DataTable, Drawer, Badge, btn, btnPrimary, btnDanger, focusRing, card, type Column } from "../console-ui";
import { toast } from "../feedback";

/* Lineage status → Badge tone + graph stroke color. Accent colors are the same
   in light/dark, so hex is fine for edges; nodes use CSS-var tokens so they
   follow the theme. */
function lineageTone(status: string): string {
  switch (status.toLowerCase()) {
    case "confirmed": return "ok";
    case "proposed": return "attention";
    case "rejected": return "blocked";
    default: return "neutral";
  }
}
const EDGE_COLOR: Record<string, string> = {
  confirmed: "#2C6E49", // moss
  proposed: "#A05E1C", // clay
  rejected: "#B23A1E", // espelette
};
const edgeColor = (status: string) => EDGE_COLOR[status.toLowerCase()] ?? "#1E6FA8";

const STATUSES = ["proposed", "confirmed", "rejected"] as const;
type StatusKey = (typeof STATUSES)[number];

function spanLabel(path: string, start: number, end: number): string {
  return `${path}:${start}-${end}`;
}
const shortName = (path: string) => path.split("/").pop() || path;

const inputCls =
  `w-full h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] text-ink placeholder:text-ink/40 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`;
const labelCls = "font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55";

const NODE_W = 190;
const NODE_H = 44;
const BIG_GRAPH = 60; // above this, require a focus/search to render

/* ── dagre auto-layout ────────────────────────────────────────────────────
   Build a directed graph over the given node ids + edges, rank it left→right,
   and translate dagre's CENTER coordinates into React Flow's top-left origin. */
function layout(nodeIds: string[], edges: LineageEdge[], seeds: Set<string>): Node[] {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "LR", nodesep: 30, ranksep: 90, marginx: 24, marginy: 24 });
  g.setDefaultEdgeLabel(() => ({}));

  const idSet = new Set(nodeIds);
  for (const id of nodeIds) g.setNode(id, { width: NODE_W, height: NODE_H });
  for (const e of edges) {
    if (idSet.has(e.fromPath) && idSet.has(e.toPath)) g.setEdge(e.fromPath, e.toPath);
  }
  dagre.layout(g);

  return nodeIds.map((id) => {
    const n = g.node(id);
    const isSeed = seeds.has(id);
    return {
      id,
      position: { x: (n?.x ?? 0) - NODE_W / 2, y: (n?.y ?? 0) - NODE_H / 2 },
      data: { label: shortName(id) },
      title: id,
      style: {
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: 11.5,
        padding: "8px 12px",
        borderRadius: 6,
        border: isSeed ? "2px solid #1E6FA8" : "1px solid rgb(var(--c-ink) / 0.25)",
        boxShadow: isSeed ? "0 0 0 3px rgb(30 111 168 / 0.15)" : undefined,
        background: "rgb(var(--c-paper))",
        color: "rgb(var(--c-ink))",
        width: NODE_W,
        textAlign: "center" as const,
        overflow: "hidden",
        whiteSpace: "nowrap" as const,
        textOverflow: "ellipsis",
      },
    } as Node;
  });
}

/* BFS over the visible edge set (treated as undirected) to gather the
   N-hop neighborhood of a set of seed nodes. */
function neighborhood(seeds: string[], edges: LineageEdge[], depth: number): Set<string> {
  const adj = new Map<string, Set<string>>();
  const add = (a: string, b: string) => {
    if (!adj.has(a)) adj.set(a, new Set());
    adj.get(a)!.add(b);
  };
  for (const e of edges) {
    add(e.fromPath, e.toPath);
    add(e.toPath, e.fromPath);
  }
  const seen = new Set<string>(seeds);
  let frontier = seeds.slice();
  for (let d = 0; d < depth; d++) {
    const next: string[] = [];
    for (const id of frontier) {
      for (const nb of adj.get(id) ?? []) {
        if (!seen.has(nb)) { seen.add(nb); next.push(nb); }
      }
    }
    frontier = next;
    if (!frontier.length) break;
  }
  return seen;
}

export function LineageGroup() {
  const [rows, setRows] = useState<LineageEdge[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [selectedId, setSelectedId] = useState<string | null>(null); // selected edge
  const [focusPath, setFocusPath] = useState<string | null>(null);   // focused node
  const [query, setQuery] = useState("");
  const [depth, setDepth] = useState(1);
  const [statusOn, setStatusOn] = useState<Record<StatusKey, boolean>>({ proposed: true, confirmed: true, rejected: true });

  const [addOpen, setAddOpen] = useState(false);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [by, setBy] = useState("human");
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .lineage()
      .then((d) => setRows(d.edges))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  // Edges passing the status filter.
  const visibleEdges = useMemo(
    () => rows.filter((e) => statusOn[e.status.toLowerCase() as StatusKey] ?? true),
    [rows, statusOn],
  );

  // Unique node paths present in the visible edge set.
  const visibleNodeIds = useMemo(() => {
    const s = new Set<string>();
    for (const e of visibleEdges) { s.add(e.fromPath); s.add(e.toPath); }
    return Array.from(s);
  }, [visibleEdges]);

  const q = query.trim().toLowerCase();

  // Seed set: an explicitly focused node, else nodes matching the search query.
  const seeds = useMemo(() => {
    if (focusPath) return [focusPath];
    if (q) return visibleNodeIds.filter((id) => id.toLowerCase().includes(q));
    return [];
  }, [focusPath, q, visibleNodeIds]);

  const focusing = seeds.length > 0 || (!!q && !!query);
  const tooBig = !focusing && visibleNodeIds.length > BIG_GRAPH;

  // The node ids + edges we actually draw.
  const { renderIds, renderEdges } = useMemo(() => {
    if (focusing) {
      const set = neighborhood(seeds, visibleEdges, depth);
      const ids = Array.from(set);
      const idSet = new Set(ids);
      return {
        renderIds: ids,
        renderEdges: visibleEdges.filter((e) => idSet.has(e.fromPath) && idSet.has(e.toPath)),
      };
    }
    if (tooBig) return { renderIds: [] as string[], renderEdges: [] as LineageEdge[] };
    return { renderIds: visibleNodeIds, renderEdges: visibleEdges };
  }, [focusing, tooBig, seeds, depth, visibleEdges, visibleNodeIds]);

  const seedSet = useMemo(() => new Set(seeds), [seeds]);

  const nodes = useMemo(() => layout(renderIds, renderEdges, seedSet), [renderIds, renderEdges, seedSet]);

  const rfEdges = useMemo<RFEdge[]>(
    () =>
      renderEdges.map((e) => {
        const color = edgeColor(e.status);
        const isSel = e.id === selectedId;
        const st = e.status.toLowerCase();
        return {
          id: e.id,
          source: e.fromPath,
          target: e.toPath,
          label: e.rel,
          animated: st === "proposed",
          labelStyle: { fontFamily: "'JetBrains Mono', monospace", fontSize: 10, fill: "rgb(var(--c-ink) / 0.6)" },
          labelBgStyle: { fill: "rgb(var(--c-paper))" },
          style: {
            stroke: color,
            strokeWidth: isSel ? 3.2 : 1.6,
            strokeDasharray: st === "rejected" ? "4 4" : undefined,
            opacity: st === "rejected" ? 0.5 : 1,
          },
          markerEnd: { type: MarkerType.ArrowClosed, color, width: 16, height: 16 },
        };
      }),
    [renderEdges, selectedId],
  );

  const selected = rows.find((r) => r.id === selectedId) ?? null;

  const onEdgeClick: EdgeMouseHandler = (_, edge) => {
    setSelectedId(edge.id);
  };
  const onNodeClick: NodeMouseHandler = (_, node) => {
    setSelectedId(null);
    setQuery("");
    setFocusPath((p) => (p === node.id ? null : node.id));
  };
  function resetView() {
    setFocusPath(null);
    setQuery("");
    setSelectedId(null);
  }

  function openAdd() {
    setFrom(""); setTo(""); setBy("human"); setNote("");
    setAddOpen(true);
  }

  async function mutate(fn: () => Promise<{ ok: boolean }>, msg: string, e?: React.MouseEvent) {
    e?.stopPropagation();
    try {
      await fn();
      toast(msg, "success");
      reload();
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    }
  }

  async function add() {
    if (!from.trim() || !to.trim()) {
      toast("From and To are required", "error");
      return;
    }
    setSaving(true);
    try {
      await api.addLineage(from.trim(), to.trim(), by, note.trim() || undefined);
      toast("Edge added", "success");
      setAddOpen(false);
      reload();
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setSaving(false);
    }
  }

  const columns: Column<LineageEdge>[] = [
    {
      key: "from", header: "From", sortable: true, sort: (r) => r.fromPath, cell: "max-w-[220px]",
      render: (r) => (
        <span className="font-term text-[12px] text-ink/85 truncate block" title={spanLabel(r.fromPath, r.fromStart, r.fromEnd)}>
          {spanLabel(r.fromPath, r.fromStart, r.fromEnd)}
        </span>
      ),
    },
    {
      key: "to", header: "To", sortable: true, sort: (r) => r.toPath, cell: "max-w-[220px]",
      render: (r) => (
        <span className="font-term text-[12px] text-ink/85 truncate block" title={spanLabel(r.toPath, r.toStart, r.toEnd)}>
          {spanLabel(r.toPath, r.toStart, r.toEnd)}
        </span>
      ),
    },
    { key: "rel", header: "Rel", render: (r) => <Badge label={r.rel} tone="neutral" /> },
    { key: "status", header: "Status", sortable: true, sort: (r) => r.status, render: (r) => <Badge label={r.status} tone={lineageTone(r.status)} /> },
    {
      key: "confidence", header: "Confidence", align: "right", sortable: true, sort: (r) => r.confidence,
      render: (r) => <span className="font-term text-[12px] text-ink/70 tabular-nums">{r.confidence.toFixed(2)}</span>,
    },
    { key: "by", header: "By", render: (r) => <span className="font-term text-[12px] text-ink/70">{r.by || "—"}</span> },
    {
      key: "actions", header: "", align: "right",
      render: (r) =>
        r.status === "proposed" ? (
          <div className="flex items-center justify-end gap-1.5">
            <button onClick={(e) => mutate(() => api.confirmLineage(r.id), "Edge confirmed", e)} className={`${btnPrimary} h-7 px-2 text-[12px]`}>Confirm</button>
            <button onClick={(e) => mutate(() => api.rejectLineage(r.id), "Edge rejected", e)} className={`${btnDanger} h-7 px-2 text-[12px]`}>Reject</button>
          </div>
        ) : (
          <span className="text-ink/30">—</span>
        ),
    },
  ];

  const hasGraph = rows.length > 0;

  return (
    <Page
      title="Lineage"
      subtitle="Span-to-span maintenance edges — search a node or filter to navigate."
      kicker="content"
      actions={
        <button onClick={openAdd} className={btnPrimary}>
          <Plus size={15} /> Add edge
        </button>
      }
    >
      {loading && rows.length === 0 && !error && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className="mt-5 rounded-md border border-espelette/30 bg-espelette/[0.05] p-4">
          <div className="text-[13px] font-medium text-espelette">Couldn’t load lineage</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">Retry</button>
        </div>
      )}

      {!error && hasGraph && (
        <div className={`${card} mt-5 overflow-hidden`}>
          {/* Toolbar */}
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-2.5 border-b border-ink/10">
            <label className="relative flex items-center">
              <Search size={14} className="absolute left-2.5 text-ink/40 pointer-events-none" />
              <input
                value={query}
                onChange={(e) => { setQuery(e.target.value); setFocusPath(null); setSelectedId(null); }}
                placeholder="Search a file…"
                className={`h-8 w-52 pl-8 pr-3 rounded-[4px] border border-ink/20 bg-paper font-term text-[12px] text-ink placeholder:text-ink/40 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`}
              />
            </label>

            <div className="flex items-center gap-1.5">
              {STATUSES.map((s) => {
                const on = statusOn[s];
                const color = edgeColor(s);
                return (
                  <button
                    key={s}
                    onClick={() => setStatusOn((m) => ({ ...m, [s]: !m[s] }))}
                    className={`inline-flex items-center gap-1.5 h-7 px-2.5 rounded-full border font-term text-[11px] transition-colors ${focusRing} ${on ? "border-ink/25 text-ink/80 bg-paper" : "border-ink/10 text-ink/35 bg-flysch/40"}`}
                    title={`${on ? "Hide" : "Show"} ${s} edges`}
                  >
                    <span className="inline-block w-2 h-2 rounded-full" style={{ background: color, opacity: on ? 1 : 0.35 }} />
                    {s}
                  </button>
                );
              })}
            </div>

            {seeds.length > 0 && (
              <div className="flex items-center gap-1.5 font-term text-[11px] text-ink/55">
                <Crosshair size={13} className="text-biscay-2" />
                <span>hops</span>
                {[1, 2].map((d) => (
                  <button
                    key={d}
                    onClick={() => setDepth(d)}
                    className={`h-6 w-6 rounded-[4px] border text-[11px] ${focusRing} ${depth === d ? "border-biscay-2 text-biscay-2 bg-biscay/[0.06]" : "border-ink/20 text-ink/60"}`}
                  >
                    {d}
                  </button>
                ))}
              </div>
            )}

            <div className="ml-auto flex items-center gap-3">
              <span className="font-term text-[11px] text-ink/55">
                {nodes.length} / {visibleNodeIds.length} nodes · {rfEdges.length} / {visibleEdges.length} edges
              </span>
              <div className="hidden sm:flex items-center gap-3 font-term text-[11px] text-ink/60">
                <LegendDot color="#2C6E49" label="confirmed" />
                <LegendDot color="#A05E1C" label="proposed" />
                <LegendDot color="#B23A1E" label="rejected" />
              </div>
              {(focusPath || query) && (
                <button onClick={resetView} className={`${btn} h-7 px-2 text-[12px]`}>
                  <RotateCcw size={13} /> Reset view
                </button>
              )}
            </div>
          </div>

          <div style={{ height: 460 }} className="relative bg-flysch/40">
            <ReactFlow
              nodes={nodes}
              edges={rfEdges}
              onEdgeClick={onEdgeClick}
              onNodeClick={onNodeClick}
              onPaneClick={resetView}
              fitView
              proOptions={{ hideAttribution: true }}
              minZoom={0.1}
              nodesDraggable
              nodesConnectable={false}
            >
              <Background color="rgb(var(--c-ink) / 0.12)" gap={18} />
              <Controls showInteractive={false} />
              <MiniMap pannable zoomable nodeColor="rgb(var(--c-biscay-2))" maskColor="rgb(var(--c-ink) / 0.08)" />
            </ReactFlow>

            {/* Overlay: too many nodes to render at once and nothing focused. */}
            {tooBig && (
              <div className="absolute inset-0 z-10 grid place-items-center p-6 pointer-events-none">
                <div className={`${card} max-w-sm p-5 text-center shadow-lg pointer-events-auto`}>
                  <Crosshair size={18} className="mx-auto text-biscay-2" />
                  <div className="mt-2 text-[13px] font-semibold text-ink">{visibleNodeIds.length} nodes</div>
                  <p className="mt-1 text-[12px] leading-relaxed text-ink/60">
                    Too many to lay out at once. Search a file above, or pick one from the table below, to focus its neighborhood.
                  </p>
                </div>
              </div>
            )}

            {/* Overlay: focused but the seed matched nothing. */}
            {!tooBig && focusing && nodes.length === 0 && (
              <div className="absolute inset-0 z-10 grid place-items-center p-6 pointer-events-none">
                <div className={`${card} max-w-sm p-5 text-center shadow-lg`}>
                  <div className="text-[13px] font-medium text-ink">No matching nodes</div>
                  <p className="mt-1 text-[12px] text-ink/60">Nothing in the current filter matches “{query || focusPath}”.</p>
                </div>
              </div>
            )}

            {/* Floating edge panel */}
            {selected && (
              <div className={`${card} absolute left-3 bottom-3 z-20 w-[300px] p-3 shadow-lg`}>
                <div className="flex items-center gap-2">
                  <Badge label={selected.status} tone={lineageTone(selected.status)} />
                  <Badge label={selected.rel} tone="neutral" />
                  <span className="ml-auto font-term text-[11px] text-ink/55">{selected.confidence.toFixed(2)}</span>
                </div>
                <div className="mt-2 font-term text-[11.5px] text-ink/80 break-all">
                  {spanLabel(selected.fromPath, selected.fromStart, selected.fromEnd)}
                  <span className="text-ink/40"> → </span>
                  {spanLabel(selected.toPath, selected.toStart, selected.toEnd)}
                </div>
                {selected.by && <div className="mt-1 font-term text-[11px] text-ink/55">by {selected.by}</div>}
                {selected.status === "proposed" && (
                  <div className="mt-3 flex items-center gap-2">
                    <button onClick={() => mutate(() => api.confirmLineage(selected.id), "Edge confirmed")} className={`${btnPrimary} h-7 px-2 text-[12px]`}>
                      <Check size={13} /> Confirm
                    </button>
                    <button onClick={() => mutate(() => api.rejectLineage(selected.id), "Edge rejected")} className={`${btnDanger} h-7 px-2 text-[12px]`}>
                      <XIcon size={13} /> Reject
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {!error && !(loading && rows.length === 0) && (
        <DataTable<LineageEdge>
          title="Edges"
          count={rows.length}
          rows={rows}
          columns={columns}
          rowKey={(r) => r.id}
          search={(r) => `${r.fromPath} ${r.toPath} ${r.rel} ${r.status} ${r.by}`}
          facet={{ label: "statuses", get: (r) => r.status }}
          onRowClick={(r) => { setSelectedId(r.id); setQuery(""); setFocusPath(r.fromPath); }}
          empty="No lineage edges yet. Add one, or run `mari lineage refine`."
        />
      )}

      <Drawer
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title="Add edge"
        subtitle="lineage"
        icon={<GitBranch size={18} className="text-biscay-2" />}
        footer={
          <button onClick={add} disabled={saving} className={`${btnPrimary} disabled:opacity-50`}>
            {saving ? "Adding…" : "Add edge"}
          </button>
        }
      >
        <div className="space-y-4">
          <label className="block">
            <div className={labelCls}>From</div>
            <input value={from} onChange={(e) => setFrom(e.target.value)} placeholder="path[#symbol]" className={`mt-1.5 font-term ${inputCls}`} />
          </label>
          <label className="block">
            <div className={labelCls}>To</div>
            <input value={to} onChange={(e) => setTo(e.target.value)} placeholder="path[#symbol]" className={`mt-1.5 font-term ${inputCls}`} />
          </label>
          <label className="block">
            <div className={labelCls}>By</div>
            <select value={by} onChange={(e) => setBy(e.target.value)} className={`mt-1.5 ${inputCls}`}>
              <option value="human">human</option>
              <option value="llm">llm</option>
            </select>
          </label>
          <label className="block">
            <div className={labelCls}>Note <span className="normal-case tracking-normal text-ink/40">(optional)</span></div>
            <input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Why couple these spans?" className={`mt-1.5 ${inputCls}`} />
          </label>
          <p className="text-[12px] text-ink/55 leading-relaxed">Both endpoints must be indexed (synced) to resolve.</p>
        </div>
      </Drawer>
    </Page>
  );
}

function LegendDot({ color, label }: { color: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className="inline-block w-2.5 h-[2px]" style={{ background: color }} />
      {label}
    </span>
  );
}
