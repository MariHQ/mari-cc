import { useEffect, useState } from "react";
import { Plus, Trash2, Sparkles, Ban, EyeOff, Bell } from "lucide-react";
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

  async function toggleZero(id: string, on: boolean) {
    try {
      await api.setZero(id, on ? "add" : "remove");
      toast(on ? `“${id}” set zero-tolerance` : `“${id}” cleared`, "success");
      await loadDetector();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to update rule", "error");
    }
  }

  async function toggleIgnore(id: string, on: boolean) {
    try {
      await api.setIgnore(id, on ? "add" : "remove");
      toast(on ? `Ignoring “${id}”` : `“${id}” un-ignored`, "success");
      await loadDetector();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to update rule", "error");
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

  const detectorColumns: Column<DetectorRule>[] = [
    {
      key: "id",
      header: "Rule id",
      sortable: true,
      sort: (r) => r.id,
      render: (r) => <span className="font-term text-[12px] text-ink/85">{r.id}</span>,
    },
    { key: "family", header: "Family", render: (r) => <Badge label={r.family} tone="neutral" /> },
    {
      key: "pack",
      header: "Pack",
      render: (r) => (
        <span className="font-term text-[11.5px] text-ink/50">{r.pack || "always-on"}</span>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: (r) => {
        if (!detector) return null;
        if (detector.zeroTolerance.includes(r.id))
          return <Badge label="zero-tolerance" tone="blocked" />;
        if (detector.ignoreRules.includes(r.id)) return <Badge label="ignored" tone="neutral" />;
        return <Badge label="active" tone="ok" />;
      },
    },
    {
      key: "actions",
      header: "",
      align: "right",
      render: (r) => {
        if (!detector) return null;
        const isZero = detector.zeroTolerance.includes(r.id);
        const isIgnored = detector.ignoreRules.includes(r.id);
        return (
          <div className="inline-flex items-center gap-1.5">
            <button
              onClick={(e) => {
                e.stopPropagation();
                toggleZero(r.id, !isZero);
              }}
              className={toggleBtn}
              title={isZero ? "Remove from zero-tolerance" : "Mark zero-tolerance"}
            >
              <Ban size={12} />
              {isZero ? "Unzero" : "Zero"}
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation();
                toggleIgnore(r.id, !isIgnored);
              }}
              className={toggleBtn}
              title={isIgnored ? "Stop ignoring" : "Ignore this rule"}
            >
              <EyeOff size={12} />
              {isIgnored ? "Unignore" : "Ignore"}
            </button>
          </div>
        );
      },
    },
  ];

  return (
    <Page
      title="Rules"
      subtitle="Edit-notify rules and the deterministic detector."
      kicker="governance"
    >
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
                className={`${btn} disabled:opacity-60`}
              >
                <Sparkles size={14} />
                {discovering ? "Discovering…" : "Discover"}
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
            <Sparkles size={15} className="text-biscay-2" />
            <h3 className="text-[15px] font-semibold text-ink">Detector rules</h3>
          </div>

          <div className={`${card} mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 px-4 py-3`}>
            <div className="flex items-center gap-1.5">
              <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">
                style guide
              </span>
              <Badge label={detector.styleGuide || "none"} tone="info" />
            </div>
            <div className="flex items-center gap-1.5">
              <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">
                grammar
              </span>
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

          <DataTable<DetectorRule>
            title="Detector rules"
            count={detector.catalog.length}
            rows={detector.catalog}
            columns={detectorColumns}
            rowKey={(r) => r.id}
            search={(r) => r.id}
            searchPlaceholder="Search rule id…"
            facet={{ label: "families", get: (r) => r.family }}
            pageSize={12}
            minW={780}
            empty="No detector rules in the catalog."
          />
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
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="api-docs"
              className={inputCls}
            />
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
