import { useEffect, useMemo, useState, type ComponentType } from "react";
import {
  Search,
  Sliders,
  RotateCcw,
  ScanLine,
  Wrench,
} from "lucide-react";
import { api, type ConfigResponse, type ConfigPath } from "@saas/lib/client";
import { Page, Badge, card, btn, btnPrimary, focusRing } from "../console-ui";
import { toast } from "../feedback";

type Scope = "repo";

/* ── Categories ──────────────────────────────────────────────────────────
   Cluster the ~100 dotted paths into a small set of friendly, ordered areas.
   A path is assigned to the FIRST category whose prefix list matches its
   leading segment(s); "Advanced" is the catch-all and always matches. */
type Category = {
  id: string;
  name: string;
  icon: ComponentType<{ size?: number | string; className?: string }>;
  /* Prefixes are matched against the dotted path's leading segment. */
  prefixes: string[];
  /* When true, this category swallows everything not yet matched. */
  catchAll?: boolean;
};

const CATEGORIES: Category[] = [
  {
    id: "detector",
    name: "Detector & style",
    icon: ScanLine,
    prefixes: ["detector", "hook", "glossary"],
  },
  { id: "advanced", name: "Advanced", icon: Wrench, prefixes: [], catchAll: true },
];

/* Match a category prefix against a path's leading segment. */
function firstSegment(path: string): string {
  const dot = path.indexOf(".");
  return dot === -1 ? path : path.slice(0, dot);
}

function categoryFor(path: string): Category {
  const seg = firstSegment(path).toLowerCase();
  for (const cat of CATEGORIES) {
    if (cat.catchAll) continue;
    for (const pre of cat.prefixes) {
      if (seg === pre || seg.startsWith(pre)) return cat;
    }
  }
  return CATEGORIES[CATEGORIES.length - 1];
}

/* Resolve a dotted path against a nested object. */
function getByPath(obj: unknown, path: string): unknown {
  if (obj == null) return undefined;
  let cur: unknown = obj;
  for (const seg of path.split(".")) {
    if (cur == null || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[seg];
    if (cur === undefined) return undefined;
  }
  return cur;
}

/* Which layer supplies the effective value for this path. */
function sourceOf(cfg: ConfigResponse, path: string): "repo" | "default" {
  if (getByPath(cfg.repo, path) !== undefined) return "repo";
  return "default";
}

/* Serialize the effective value into the string an <input>/<textarea> shows. */
function toEditString(type: string, value: unknown): string {
  if (value === undefined || value === null) {
    if (type === "boolean") return "false";
    if (type === "array") return "[]";
    if (type === "object") return "{}";
    return "";
  }
  if (type === "array" || type === "object") {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
  if (type === "boolean") return value === true ? "true" : "false";
  return String(value);
}

const inputCls =
  `w-full h-8 px-2 rounded-[4px] border border-ink/20 bg-paper font-term text-[12.5px] text-ink outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`;

function ConfigRow({
  path,
  type,
  cfg,
  scope,
  onSaved,
}: {
  path: string;
  type: string;
  cfg: ConfigResponse;
  scope: Scope;
  onSaved: () => void;
}) {
  const effective = getByPath(cfg.effective, path);
  const initial = useMemo(() => toEditString(type, effective), [type, effective]);
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);

  // Reset local draft whenever the underlying value changes (e.g. after reload).
  useEffect(() => setDraft(initial), [initial]);

  const changed = draft !== initial;
  const src = sourceOf(cfg, path);
  const srcTone = src === "repo" ? "info" : "muted";

  async function save() {
    let value: unknown;
    try {
      if (type === "boolean") {
        value = draft === "true";
      } else if (type === "integer" || type === "number") {
        const n = Number(draft);
        if (draft.trim() === "" || Number.isNaN(n)) {
          toast(`"${path}" must be a valid number`, "error");
          return;
        }
        value = type === "integer" ? Math.trunc(n) : n;
      } else if (type === "array" || type === "object") {
        value = JSON.parse(draft);
      } else {
        value = draft;
      }
    } catch (e: unknown) {
      toast(`Invalid JSON for "${path}": ${e instanceof Error ? e.message : String(e)}`, "error");
      return;
    }

    setSaving(true);
    try {
      const res = await api.setConfig(path, value, scope);
      toast("Saved", "success");
      void res;
      onSaved();
    } catch (e: unknown) {
      toast(e instanceof Error ? e.message : String(e), "error");
    } finally {
      setSaving(false);
    }
  }

  function reset() {
    setDraft(initial);
  }

  let control;
  if (type === "boolean") {
    control = (
      <select value={draft} onChange={(e) => setDraft(e.target.value)} className={inputCls}>
        <option value="true">true</option>
        <option value="false">false</option>
      </select>
    );
  } else if (type === "array" || type === "object") {
    control = (
      <textarea
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        spellCheck={false}
        rows={Math.min(8, Math.max(2, draft.split("\n").length))}
        className={`${inputCls} h-auto py-1.5 leading-[1.5] resize-y`}
      />
    );
  } else if (type === "integer" || type === "number") {
    control = (
      <input
        type="number"
        step={type === "integer" ? 1 : "any"}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        className={inputCls}
      />
    );
  } else {
    control = (
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        className={inputCls}
      />
    );
  }

  return (
    <div className="grid grid-cols-1 sm:grid-cols-[minmax(180px,1fr)_minmax(200px,1.4fr)] gap-x-4 gap-y-2 px-4 py-3 border-b border-ink/10 last:border-0 items-start">
      <div className="min-w-0 pt-1">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-term text-[12.5px] text-ink/90 break-all">{path}</span>
          <Badge label={src} tone={srcTone} />
          <span className="font-term text-[10px] uppercase tracking-[0.08em] text-ink/40">{type}</span>
        </div>
      </div>
      <div className="min-w-0">
        {control}
        {changed && (
          <div className="mt-2 flex items-center gap-2">
            <button onClick={save} disabled={saving} className={`${btnPrimary} h-7 px-2.5 text-[12px] disabled:opacity-60`}>
              {saving ? "Saving…" : "Save"}
            </button>
            <button onClick={reset} disabled={saving} className={`${btn} h-7 px-2 text-[12px]`} title="Discard edit">
              <RotateCcw size={12} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export function ConfigGroup() {
  const [data, setData] = useState<ConfigResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const scope: Scope = "repo";
  const [query, setQuery] = useState("");
  const [activeId, setActiveId] = useState<string>(CATEGORIES[0].id);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .config()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  /* Paths surviving the search filter. */
  const filtered = useMemo(() => {
    if (!data) return [] as ConfigPath[];
    const q = query.trim().toLowerCase();
    return q ? data.paths.filter((p) => p.path.toLowerCase().includes(q)) : data.paths;
  }, [data, query]);

  /* Cluster filtered paths by category, preserving CATEGORIES order and
     keeping only categories that hold ≥1 path after filtering. */
  const buckets = useMemo(() => {
    const byId = new Map<string, ConfigPath[]>();
    for (const p of filtered) {
      const cat = categoryFor(p.path);
      const list = byId.get(cat.id);
      if (list) list.push(p);
      else byId.set(cat.id, [p]);
    }
    return CATEGORIES.map((cat) => ({ cat, paths: byId.get(cat.id) ?? [] })).filter(
      (b) => b.paths.length > 0,
    );
  }, [filtered]);

  /* Keep the active category valid as the filter narrows the visible set. */
  useEffect(() => {
    if (buckets.length === 0) return;
    if (!buckets.some((b) => b.cat.id === activeId)) {
      setActiveId(buckets[0].cat.id);
    }
  }, [buckets, activeId]);

  const active = buckets.find((b) => b.cat.id === activeId) ?? buckets[0];

  return (
    <Page
      title="Config"
      subtitle="Repository configuration from .mari/config.json."
      kicker="system"
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load config</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && (
        <>
          <div className="mt-5 flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-1.5 h-9 px-2.5 rounded-[4px] border border-ink/20 bg-paper focus-within:border-biscay-2 focus-within:ring-1 focus-within:ring-biscay-2/40">
              <Search size={14} className="text-ink/50" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Filter paths…"
                className="w-[180px] sm:w-[240px] bg-transparent font-term text-[12.5px] text-ink placeholder:text-ink/45 outline-none"
              />
            </div>
            <span className="font-term text-[11.5px] text-ink/50">
              {filtered.length} of {data.paths.length} settings · writing to{" "}
              <span className="text-ink/80 font-medium">{scope}</span>
            </span>
          </div>

          {buckets.length === 0 ? (
            <div className={`${card} mt-5 p-6 text-center`}>
              <Sliders size={22} className="mx-auto text-ink/25" />
              <div className="mt-2 text-[13px] text-ink/70">
                {data.paths.length === 0 ? "No configuration settings." : "No paths match your filter."}
              </div>
            </div>
          ) : (
            <div className="mt-5 grid grid-cols-1 md:grid-cols-[200px_minmax(0,1fr)] gap-4 md:gap-5 items-start">
              {/* ── Category rail ─────────────────────────────────────── */}
              <nav className="md:sticky md:top-4">
                <div className="font-term text-[10px] uppercase tracking-[0.1em] text-ink/40 px-2 mb-1.5">
                  Categories
                </div>
                <ul className="flex flex-col gap-0.5">
                  {buckets.map(({ cat, paths }) => {
                    const Icon = cat.icon;
                    const on = active?.cat.id === cat.id;
                    return (
                      <li key={cat.id}>
                        <button
                          onClick={() => setActiveId(cat.id)}
                          className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-[4px] text-left transition-colors ${focusRing} ${
                            on
                              ? "bg-biscay/[0.08] border border-biscay-2/30 text-ink"
                              : "border border-transparent text-ink/70 hover:bg-flysch/60 hover:text-ink"
                          }`}
                        >
                          <Icon
                            size={14}
                            className={on ? "text-biscay-2 shrink-0" : "text-ink/45 shrink-0"}
                          />
                          <span className="text-[12.5px] font-medium flex-1 min-w-0 truncate">
                            {cat.name}
                          </span>
                          <span
                            className={`font-term text-[10.5px] font-medium rounded-[3px] px-1.5 py-0.5 ${
                              on
                                ? "text-biscay-2 bg-biscay/[0.1]"
                                : "text-ink/55 bg-flysch border border-ink/10"
                            }`}
                          >
                            {paths.length}
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </nav>

              {/* ── Active category settings ──────────────────────────── */}
              <div className={`${card} overflow-hidden`}>
                {active && (
                  <>
                    <div className="flex items-center gap-2 px-4 py-3 border-b border-ink/10 bg-flysch/40">
                      <active.cat.icon size={15} className="text-biscay-2" />
                      <h4 className="text-[14px] font-semibold text-ink">{active.cat.name}</h4>
                      <span className="font-term text-[11px] font-medium text-ink/55 bg-paper border border-ink/10 rounded-[3px] px-1.5 py-0.5">
                        {active.paths.length}
                      </span>
                    </div>
                    <div>
                      {active.paths.map((p) => (
                        <ConfigRow
                          key={p.path}
                          path={p.path}
                          type={p.type}
                          cfg={data}
                          scope={scope}
                          onSaved={reload}
                        />
                      ))}
                    </div>
                  </>
                )}
              </div>
            </div>
          )}
        </>
      )}
    </Page>
  );
}
