import { useEffect, useMemo, useState } from "react";
import { ListChecks, RotateCcw, Save, Search } from "lucide-react";
import { api, type DetectorList } from "@saas/lib/client";
import { card, btn, btnPrimary, focusRing, Badge } from "../console-ui";
import { toast } from "../feedback";

/* A word/phrase list edits as one entry per line; map/weighted/groups edit as
   JSON rows (the same shape the config file stores) with an inline hint. */
function isLineList(kind: DetectorList["kind"]): boolean {
  return kind === "words" || kind === "phrases";
}

function toEditor(list: DetectorList): string {
  const value = (list.override ?? list.default) as unknown[];
  if (isLineList(list.kind)) return (value as string[]).join("\n");
  return JSON.stringify(value, null, 2);
}

/* Parse the editor text back into the JSON array the API expects, or throw. */
function fromEditor(kind: DetectorList["kind"], text: string): unknown[] {
  if (isLineList(kind)) {
    return text
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
  }
  const parsed = JSON.parse(text);
  if (!Array.isArray(parsed)) throw new Error("expected a JSON array");
  return parsed;
}

const KIND_HINT: Record<DetectorList["kind"], string> = {
  words: "one word per line",
  phrases: "one phrase per line",
  weighted: 'JSON rows: ["word", "base form", weight]',
  map: 'JSON rows: ["from", "to"]',
  groups: 'JSON rows: ["variant a", "variant b", …]',
};

function familyTone(family: string): string {
  if (family === "ai-slop") return "blocked";
  if (family === "clarity") return "attention";
  if (family === "style") return "info";
  return "neutral";
}

function ListRow({ list, scope, onSaved }: { list: DetectorList; scope: "repo" | "global"; onSaved: () => void }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState(() => toEditor(list));
  const [busy, setBusy] = useState(false);

  useEffect(() => setText(toEditor(list)), [list]);

  const count = ((list.override ?? list.default) as unknown[]).length;

  async function save() {
    setBusy(true);
    try {
      const value = fromEditor(list.kind, text);
      await api.setDetectorList(list.id, value, scope);
      toast(`Saved “${list.label}” (${value.length} entries)`, "success");
      onSaved();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  async function reset() {
    setBusy(true);
    try {
      await api.resetDetectorList(list.id, scope);
      toast(`Reset “${list.label}” to the built-in default`, "success");
      onSaved();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="border-b border-ink/10 last:border-0">
      <button
        onClick={() => setOpen((o) => !o)}
        className={`w-full flex items-center gap-2 px-4 py-2.5 text-left hover:bg-flysch/40 ${focusRing}`}
      >
        <span className="text-[13px] font-medium text-ink">{list.label}</span>
        <span className="font-term text-[11px] text-ink/45">{list.id}</span>
        <Badge label={list.family} tone={familyTone(list.family)} />
        {list.pack && <Badge label={list.pack} tone="neutral" />}
        <span className="ml-auto flex items-center gap-2">
          {list.overridden && (
            <span className="font-term text-[10.5px] text-clay bg-clay/[0.12] rounded-[3px] px-1.5 py-0.5">
              overridden · {list.source}
            </span>
          )}
          <span className="font-term text-[11px] text-ink/50">{count}</span>
        </span>
      </button>
      {open && (
        <div className="px-4 pb-3.5 pt-1">
          <div className="mb-1.5 font-term text-[11px] text-ink/50">
            {list.kind} — {KIND_HINT[list.kind]}
          </div>
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={Math.min(16, Math.max(4, text.split("\n").length + 1))}
            spellCheck={false}
            className={`w-full px-3 py-2.5 rounded-[4px] border border-ink/20 bg-paper font-term text-[12.5px] leading-[1.6] text-ink outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 resize-y ${focusRing}`}
          />
          <div className="mt-2 flex items-center gap-2">
            <button onClick={save} disabled={busy} className={`${btnPrimary} h-8 px-2.5 text-[12.5px] disabled:opacity-60`}>
              <Save size={13} /> Save to {scope}
            </button>
            <button
              onClick={reset}
              disabled={busy || !list.overridden}
              title={list.overridden ? "Revert to the built-in list" : "No override to reset"}
              className={`${btn} h-8 px-2.5 text-[12.5px] disabled:opacity-40`}
            >
              <RotateCcw size={13} /> Reset
            </button>
            <button
              onClick={() => setText(JSON.stringify(list.default, null, isLineList(list.kind) ? 0 : 2))}
              className="ml-auto font-term text-[11px] text-biscay-2 hover:underline"
              title="Fill the editor with the built-in default (not yet saved)"
            >
              load default
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export function DetectorLists() {
  const [lists, setLists] = useState<DetectorList[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<"repo" | "global">("repo");

  async function load() {
    try {
      const r = await api.detectorLists();
      setLists(r.lists);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }
  useEffect(() => {
    void load();
  }, []);

  const filtered = useMemo(() => {
    if (!lists) return [];
    const q = query.trim().toLowerCase();
    if (!q) return lists;
    return lists.filter(
      (l) => l.label.toLowerCase().includes(q) || l.id.includes(q) || l.family.includes(q) || (l.pack ?? "").includes(q),
    );
  }, [lists, query]);

  const overridden = lists?.filter((l) => l.overridden).length ?? 0;

  return (
    <div className={`${card} mt-6 overflow-hidden`}>
      <div className="flex items-center gap-2 px-4 py-3 border-b border-ink/10">
        <ListChecks size={15} className="text-biscay-2" />
        <h4 className="text-[14px] font-semibold text-ink">Word lists</h4>
        <span className="font-term text-[11px] text-ink/55">
          {lists?.length ?? 0} lists{overridden > 0 ? ` · ${overridden} overridden` : ""}
        </span>
        <div className="ml-auto flex items-center gap-2">
          <div className="inline-flex rounded-[4px] border border-ink/20 overflow-hidden">
            {(["repo", "global"] as const).map((sc) => (
              <button
                key={sc}
                onClick={() => setScope(sc)}
                className={`h-8 px-2.5 text-[12px] font-medium ${focusRing} ${
                  scope === sc ? "bg-biscay text-white" : "text-ink/65 hover:text-ink"
                }`}
                title={sc === "repo" ? "Write to <repo>/.mari/config.json (team-shared)" : "Write to ~/.mari/config.json (personal)"}
              >
                {sc}
              </button>
            ))}
          </div>
          <div className="relative">
            <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-ink/40" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="filter lists…"
              className={`h-8 w-44 pl-7 pr-2 rounded-[4px] border border-ink/20 bg-paper font-term text-[12px] text-ink outline-none focus:border-biscay-2 ${focusRing}`}
            />
          </div>
        </div>
      </div>
      <p className="px-4 pt-2.5 text-[12.5px] text-ink/60">
        Every list the detector triggers on. Editing a list <span className="font-medium">replaces</span> the built-in set
        wholesale in the chosen config layer; an empty list disables its rule. Reset reverts to the built-in.
      </p>
      {error && <div className="px-4 py-3 text-[12.5px] text-espelette">{error}</div>}
      {lists === null && !error && <div className="px-4 py-4 text-[13px] text-ink/50">Loading…</div>}
      <div className="mt-1 max-h-[620px] overflow-y-auto">
        {filtered.map((l) => (
          <ListRow key={l.id} list={l} scope={scope} onSaved={load} />
        ))}
        {lists !== null && filtered.length === 0 && (
          <div className="px-4 py-4 text-[13px] text-ink/50">No lists match “{query}”.</div>
        )}
      </div>
    </div>
  );
}
