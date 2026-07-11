import { useEffect, useMemo, useState, type ComponentType } from "react";
import {
  Plus,
  Trash2,
  Sparkles,
  Ban,
  EyeOff,
  Bell,
  Search,
  List,
  Eye,
  Type,
  Users,
  Anchor,
  Tag as TagIcon,
  ShieldCheck,
} from "lucide-react";
import { api } from "@saas/lib/client";
import type { EditRule, DetectorInfo, DetectorRule } from "@saas/lib/client";
import { Page, DataTable, Drawer, Badge, btn, btnPrimary, btnDanger, card, focusRing } from "../console-ui";
import type { Column } from "../console-ui";
import { toast } from "../feedback";

const inputCls =
  "w-full h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 " +
  focusRing;

function Label({ children }: { children: React.ReactNode }) {
  return (
    <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55 mb-1.5">{children}</div>
  );
}

function Chips({ items }: { items: string[] }) {
  if (!items || items.length === 0) return <span className="text-ink/40">—</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((s, i) => (
        <span
          key={i}
          className="font-term text-[11px] text-ink/70 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5"
        >
          {s}
        </span>
      ))}
    </div>
  );
}

const toggleBtn = `inline-flex items-center gap-1 h-7 px-2 rounded-[4px] border border-ink/20 bg-paper text-[12px] font-medium text-ink/75 hover:border-ink/45 hover:text-ink transition-colors ${focusRing}`;

/* Icon per detector family (falls back to a tag). */
const FAMILY_ICON: Record<string, ComponentType<{ size?: number | string; className?: string }>> = {
  "ai-slop": Sparkles,
  clarity: Eye,
  style: Type,
  inclusive: Users,
  grounding: Anchor,
};
const familyIcon = (f: string) => FAMILY_ICON[f] ?? TagIcon;

/* ── Detector rules: clustered by family (+ quick views), same rail as Config ── */
function DetectorRules({ detector, onChanged }: { detector: DetectorInfo; onChanged: () => void }) {
  const [activeId, setActiveId] = useState<string>("all");
  const [query, setQuery] = useState("");

  const zero = useMemo(() => new Set(detector.zeroTolerance), [detector.zeroTolerance]);
  const ignored = useMemo(() => new Set(detector.ignoreRules), [detector.ignoreRules]);

  /* Families present in the catalog, with counts. */
  const families = useMemo(() => {
    const counts = new Map<string, number>();
    for (const r of detector.catalog) counts.set(r.family, (counts.get(r.family) ?? 0) + 1);
    return Array.from(counts.entries())
      .map(([id, count]) => ({ id, count }))
      .sort((a, b) => a.id.localeCompare(b.id));
  }, [detector.catalog]);

  /* Rail: quick views first, then one entry per family. */
  const quick = [
    { id: "all", name: "All rules", icon: List, count: detector.catalog.length },
    { id: "zero", name: "Zero-tolerance", icon: Ban, count: zero.size },
    { id: "ignored", name: "Ignored", icon: EyeOff, count: ignored.size },
  ];

  const rulesInCat = (catId: string): DetectorRule[] => {
    if (catId === "all") return detector.catalog;
    if (catId === "zero") return detector.catalog.filter((r) => zero.has(r.id));
    if (catId === "ignored") return detector.catalog.filter((r) => ignored.has(r.id));
    if (catId.startsWith("fam:")) return detector.catalog.filter((r) => r.family === catId.slice(4));
    return detector.catalog;
  };

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const base = rulesInCat(activeId);
    return q ? base.filter((r) => r.id.toLowerCase().includes(q)) : base;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, query, detector]);

  const activeName =
    quick.find((v) => v.id === activeId)?.name ??
    (activeId.startsWith("fam:") ? activeId.slice(4) : "Rules");
  const ActiveIcon =
    quick.find((v) => v.id === activeId)?.icon ??
    (activeId.startsWith("fam:") ? familyIcon(activeId.slice(4)) : List);

  async function toggleZero(id: string, on: boolean) {
    try {
      await api.setZero(id, on ? "add" : "remove");
      toast(on ? `“${id}” set zero-tolerance` : `“${id}” cleared`, "success");
      onChanged();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to update rule", "error");
    }
  }
  async function toggleIgnore(id: string, on: boolean) {
    try {
      await api.setIgnore(id, on ? "add" : "remove");
      toast(on ? `Ignoring “${id}”` : `“${id}” un-ignored`, "success");
      onChanged();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to update rule", "error");
    }
  }

  const railItem = (
    item: { id: string; name: string; icon: ComponentType<{ size?: number | string; className?: string }>; count: number },
  ) => {
    const Icon = item.icon;
    const on = activeId === item.id;
    return (
      <li key={item.id}>
        <button
          onClick={() => setActiveId(item.id)}
          className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-[4px] text-left transition-colors ${focusRing} ${
            on
              ? "bg-biscay/[0.08] border border-biscay-2/30 text-ink"
              : "border border-transparent text-ink/70 hover:bg-flysch/60 hover:text-ink"
          }`}
        >
          <Icon size={14} className={on ? "text-biscay-2 shrink-0" : "text-ink/45 shrink-0"} />
          <span className="text-[12.5px] font-medium flex-1 min-w-0 truncate">{item.name}</span>
          <span
            className={`font-term text-[10.5px] font-medium rounded-[3px] px-1.5 py-0.5 ${
              on ? "text-biscay-2 bg-biscay/[0.1]" : "text-ink/55 bg-flysch border border-ink/10"
            }`}
          >
            {item.count}
          </span>
        </button>
      </li>
    );
  };

  return (
    <div className="mt-5 grid grid-cols-1 md:grid-cols-[200px_minmax(0,1fr)] gap-4 md:gap-5 items-start">
      {/* Rail */}
      <nav className="md:sticky md:top-4">
        <div className="font-term text-[10px] uppercase tracking-[0.1em] text-ink/40 px-2 mb-1.5">Views</div>
        <ul className="flex flex-col gap-0.5">{quick.map(railItem)}</ul>
        <div className="font-term text-[10px] uppercase tracking-[0.1em] text-ink/40 px-2 mb-1.5 mt-4">
          Families
        </div>
        <ul className="flex flex-col gap-0.5">
          {families.map((f) => railItem({ id: `fam:${f.id}`, name: f.id, icon: familyIcon(f.id), count: f.count }))}
        </ul>
      </nav>

      {/* Active list */}
      <div className={`${card} overflow-hidden`}>
        <div className="flex items-center gap-2 px-4 py-3 border-b border-ink/10 bg-flysch/40">
          <ActiveIcon size={15} className="text-biscay-2" />
          <h4 className="text-[14px] font-semibold text-ink">{activeName}</h4>
          <span className="font-term text-[11px] font-medium text-ink/55 bg-paper border border-ink/10 rounded-[3px] px-1.5 py-0.5">
            {rows.length}
          </span>
          <div className="ml-auto flex items-center gap-1.5 h-8 px-2.5 rounded-[4px] border border-ink/20 bg-paper focus-within:border-biscay-2 focus-within:ring-1 focus-within:ring-biscay-2/40">
            <Search size={13} className="text-ink/50" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search rule id…"
              className="w-[120px] sm:w-[160px] bg-transparent font-term text-[12px] text-ink placeholder:text-ink/45 outline-none"
            />
          </div>
        </div>

        {rows.length === 0 ? (
          <div className="py-14 text-center text-[13px] text-ink/55">
            {query ? "No rules match your search." : "No rules here."}
          </div>
        ) : (
          <div className="max-h-[560px] overflow-y-auto">
            {rows.map((r) => {
              const isZero = zero.has(r.id);
              const isIgnored = ignored.has(r.id);
              return (
                <div
                  key={r.id}
                  className="flex items-center gap-3 px-4 py-2.5 border-b border-ink/10 last:border-0"
                >
                  <span className="font-term text-[12.5px] text-ink/90 flex-1 min-w-0 truncate" title={r.id}>
                    {r.id}
                  </span>
                  <Badge label={r.family} tone="neutral" />
                  <span className="font-term text-[11px] text-ink/45 hidden lg:block w-[72px] truncate">
                    {r.pack || "always-on"}
                  </span>
                  {isZero ? (
                    <Badge label="zero" tone="blocked" />
                  ) : isIgnored ? (
                    <Badge label="ignored" tone="neutral" />
                  ) : (
                    <Badge label="active" tone="ok" />
                  )}
                  <div className="flex items-center gap-1.5 shrink-0">
                    <button
                      onClick={() => toggleZero(r.id, !isZero)}
                      className={toggleBtn}
                      title={isZero ? "Remove from zero-tolerance" : "Mark zero-tolerance"}
                    >
                      <Ban size={12} />
                      {isZero ? "Unzero" : "Zero"}
                    </button>
                    <button
                      onClick={() => toggleIgnore(r.id, !isIgnored)}
                      className={toggleBtn}
                      title={isIgnored ? "Stop ignoring" : "Ignore this rule"}
                    >
                      <EyeOff size={12} />
                      {isIgnored ? "Unignore" : "Ignore"}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

export function RulesGroup() {
  const [rules, setRules] = useState<EditRule[] | null>(null);
  const [detector, setDetector] = useState<DetectorInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [paths, setPaths] = useState("");
  const [notify, setNotify] = useState("");
  const [exclude, setExclude] = useState("");
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);

  function loadRules() {
    return api.rules().then((d) => setRules(d.rules));
  }
  function loadDetector() {
    return api.detector().then((d) => setDetector(d));
  }

  function reload() {
    setLoading(true);
    setError(null);
    Promise.all([loadRules(), loadDetector()])
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  function openDrawer() {
    setName("");
    setPaths("");
    setNotify("");
    setExclude("");
    setOpen(true);
  }

  async function submit() {
    if (!name.trim() || !paths.trim() || !notify.trim()) {
      toast("Name, Paths, and Notify are required", "error");
      return;
    }
    setBusy(true);
    try {
      await api.addRule({
        name: name.trim(),
        paths: paths.trim(),
        notify: notify.trim(),
        exclude: exclude.trim() || undefined,
      });
      toast(`Added rule “${name.trim()}”`, "success");
      setOpen(false);
      await loadRules();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to add rule", "error");
    } finally {
      setBusy(false);
    }
  }

  async function removeRule(n: string) {
    try {
      await api.removeRule(n);
      toast(`Removed rule “${n}”`, "success");
      await loadRules();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to remove rule", "error");
    }
  }

  async function discover() {
    setDiscovering(true);
    try {
      const d = await api.discoverRules();
      setRules(d.rules);
      toast(`${d.rules.length} rule${d.rules.length === 1 ? "" : "s"} discovered`, "success");
    } catch (e) {
      toast(e instanceof Error ? e.message : "Discovery failed", "error");
    } finally {
      setDiscovering(false);
    }
  }

  const ruleColumns: Column<EditRule>[] = [
    {
      key: "name",
      header: "Name",
      sortable: true,
      sort: (r) => r.name,
      render: (r) => <span className="font-medium text-ink">{r.name}</span>,
    },
    { key: "paths", header: "Paths", render: (r) => <Chips items={r.paths} /> },
    {
      key: "notify",
      header: "Notify",
      render: (r) => <span className="text-[13px] text-ink/80">{r.notify || "—"}</span>,
    },
    { key: "exclude", header: "Exclude", render: (r) => <Chips items={r.exclude} /> },
    {
      key: "actions",
      header: "",
      align: "right",
      render: (r) => (
        <button
          onClick={(e) => {
            e.stopPropagation();
            removeRule(r.name);
          }}
          className={`${btnDanger} h-7 px-2 text-[12px]`}
        >
          <Trash2 size={13} />
          Remove
        </button>
      ),
    },
  ];

  return (
    <Page title="Rules" subtitle="Edit-notify rules and the deterministic detector." kicker="governance">
      {loading && !rules && !detector && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load rules</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {!error && rules && (
        <>
          <div className="mt-6 flex items-center gap-2">
            <Bell size={15} className="text-biscay-2" />
            <h3 className="text-[15px] font-semibold text-ink">Edit-notify rules</h3>
            <div className="ml-auto flex items-center gap-2">
              <button
                onClick={discover}
                disabled={discovering}
                title="Auto-detect edit-notify rules from the repo layout (e.g. src/ ↔ docs drift, changed manifests) and add them. Same as `mari rules discover --write`."
                className={`${btn} disabled:opacity-60`}
              >
                <Sparkles size={14} />
                {discovering ? "Detecting…" : "Auto-detect"}
              </button>
              <button onClick={openDrawer} className={btnPrimary}>
                <Plus size={14} />
                Add rule
              </button>
            </div>
          </div>

          <DataTable<EditRule>
            title="Edit-notify rules"
            count={rules.length}
            rows={rules}
            columns={ruleColumns}
            rowKey={(r) => r.name}
            search={(r) => `${r.name} ${r.notify} ${r.paths.join(" ")}`}
            searchPlaceholder="Search rules…"
            pageSize={8}
            minW={820}
            empty="No edit-notify rules yet. These fire the post-edit hook when matching files change — add one or run Discover."
          />
        </>
      )}

      {!error && detector && (
        <>
          <div className="mt-8 flex items-center gap-2">
            <ShieldCheck size={15} className="text-biscay-2" />
            <h3 className="text-[15px] font-semibold text-ink">Detector rules</h3>
          </div>

          <div className={`${card} mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 px-4 py-3`}>
            <div className="flex items-center gap-1.5">
              <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">style guide</span>
              <Badge label={detector.styleGuide || "none"} tone="info" />
            </div>
            <div className="flex items-center gap-1.5">
              <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">grammar</span>
              <Badge label={detector.grammar ? "on" : "off"} tone={detector.grammar ? "ok" : "neutral"} />
            </div>
            <div className="font-term text-[12px] text-ink/60">
              <span className="text-ink/85 font-medium">{detector.catalog.length}</span> rules
            </div>
            <div className="font-term text-[12px] text-ink/60">
              <span className="text-espelette font-medium">{detector.zeroTolerance.length}</span> zero-tolerance
            </div>
            <div className="font-term text-[12px] text-ink/60">
              <span className="text-ink/85 font-medium">{detector.ignoreRules.length}</span> ignored
            </div>
          </div>

          <DetectorRules detector={detector} onChanged={loadDetector} />
        </>
      )}

      <Drawer
        open={open}
        onClose={() => setOpen(false)}
        title="Add edit-notify rule"
        subtitle="post-edit hook"
        icon={<Bell size={18} className="text-biscay-2" />}
        footer={
          <button
            onClick={submit}
            disabled={busy}
            className={`${btnPrimary} w-full justify-center disabled:opacity-60`}
          >
            <Plus size={14} />
            {busy ? "Adding…" : "Add rule"}
          </button>
        }
      >
        <div className="flex flex-col gap-4">
          <div>
            <Label>Name</Label>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="api-docs" className={inputCls} />
          </div>
          <div>
            <Label>Paths</Label>
            <input
              value={paths}
              onChange={(e) => setPaths(e.target.value)}
              placeholder="src/api/**, docs/api/*.md"
              className={`${inputCls} font-term`}
            />
            <div className="mt-1 font-term text-[11px] text-ink/50">Comma-separated globs.</div>
          </div>
          <div>
            <Label>Notify</Label>
            <input
              value={notify}
              onChange={(e) => setNotify(e.target.value)}
              placeholder="Remember to update the API docs."
              className={inputCls}
            />
          </div>
          <div>
            <Label>Exclude (optional)</Label>
            <input
              value={exclude}
              onChange={(e) => setExclude(e.target.value)}
              placeholder="**/*.test.ts"
              className={`${inputCls} font-term`}
            />
            <div className="mt-1 font-term text-[11px] text-ink/50">Comma-separated globs.</div>
          </div>
        </div>
      </Drawer>
    </Page>
  );
}
