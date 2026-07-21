import { useEffect, useState } from "react";
import { Plus, Trash2, Link2, X } from "lucide-react";
import { api } from "@saas/lib/client";
import type { Nudge } from "@saas/lib/client";
import { Page, DataTable, Drawer, btn, btnPrimary, btnDanger, card, focusRing } from "../ui";
import type { Column } from "../ui";
import { toast } from "../feedback";

const inputCls =
  "w-full h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 " +
  focusRing;

/** Turn a {path, symbol?} endpoint into a compact `path#symbol` string. */
function endpoint(e: unknown): string {
  if (e && typeof e === "object") {
    const o = e as Record<string, unknown>;
    if (typeof o.path === "string") {
      return typeof o.symbol === "string" && o.symbol ? `${o.path}#${o.symbol}` : o.path;
    }
  }
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

function Label({ children }: { children: React.ReactNode }) {
  return (
    <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55 mb-1.5">{children}</div>
  );
}

export function NudgesGroup() {
  const [data, setData] = useState<{ nudges: Nudge[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [when, setWhen] = useState("");
  const [targets, setTargets] = useState<string[]>([""]);
  const [message, setMessage] = useState("");
  const [exclude, setExclude] = useState("");
  const [busy, setBusy] = useState(false);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .nudges()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  function openDrawer() {
    setName("");
    setWhen("");
    setTargets([""]);
    setMessage("");
    setExclude("");
    setOpen(true);
  }

  async function remove(n: string) {
    try {
      await api.removeNudge(n);
      toast(`Removed nudge “${n}”`, "success");
      reload();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to remove nudge", "error");
    }
  }

  async function submit() {
    const edit = targets.map((t) => t.trim()).filter(Boolean);
    if (!name.trim() || !when.trim() || edit.length === 0) {
      toast("Name, When, and at least one edit target are required", "error");
      return;
    }
    setBusy(true);
    try {
      await api.addNudge({
        name: name.trim(),
        when: when.trim(),
        edit,
        message: message.trim() || undefined,
        exclude: exclude.trim() || undefined,
      });
      toast(`Added nudge “${name.trim()}”`, "success");
      setOpen(false);
      reload();
    } catch (e) {
      toast(e instanceof Error ? e.message : "Failed to add nudge", "error");
    } finally {
      setBusy(false);
    }
  }

  const columns: Column<Nudge>[] = [
    {
      key: "name",
      header: "Name",
      sortable: true,
      sort: (r) => r.name,
      render: (r) => <span className="font-medium text-ink">{r.name}</span>,
    },
    {
      key: "when",
      header: "When",
      render: (r) => <span className="font-term text-[12px] text-ink/80">{endpoint(r.when)}</span>,
    },
    {
      key: "edit",
      header: "Edit targets",
      render: (r) => (
        <div className="flex flex-wrap gap-1">
          {r.edit.map((e, i) => (
            <span
              key={i}
              className="font-term text-[11px] text-ink/70 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5"
            >
              {endpoint(e)}
            </span>
          ))}
        </div>
      ),
    },
    {
      key: "message",
      header: "Message",
      render: (r) => <span className="text-[13px] text-ink/80">{r.message || "—"}</span>,
    },
    {
      key: "actions",
      header: "",
      align: "right",
      render: (r) => (
        <button
          onClick={(e) => {
            e.stopPropagation();
            remove(r.name);
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
    <Page
      title="Nudges"
      subtitle="When this changes, remember to update that — hand-declared maintenance couplings."
      kicker="governance"
      actions={
        <button onClick={openDrawer} className={btnPrimary}>
          <Plus size={14} />
          Add nudge
        </button>
      }
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load nudges</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && (
        <DataTable<Nudge>
          title="Nudges"
          count={data.nudges.length}
          rows={data.nudges}
          columns={columns}
          rowKey={(r) => r.name}
          search={(r) => r.name}
          searchPlaceholder="Search nudges…"
          pageSize={10}
          minW={860}
          empty="No nudges yet. Add one to be reminded when coupled files drift."
        />
      )}

      <Drawer
        open={open}
        onClose={() => setOpen(false)}
        title="Add nudge"
        subtitle="maintenance coupling"
        icon={<Link2 size={18} className="text-biscay-2" />}
        footer={
          <button
            onClick={submit}
            disabled={busy}
            className={`${btnPrimary} w-full justify-center disabled:opacity-60`}
          >
            <Plus size={14} />
            {busy ? "Adding…" : "Add nudge"}
          </button>
        }
      >
        <div className="flex flex-col gap-4">
          <div>
            <Label>Name</Label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="handler-schema-sync"
              className={inputCls}
            />
          </div>

          <div>
            <Label>When</Label>
            <input
              value={when}
              onChange={(e) => setWhen(e.target.value)}
              placeholder="src/api/*.rs#handler"
              className={`${inputCls} font-term`}
            />
            <div className="mt-1 font-term text-[11px] text-ink/50">
              Glob with optional <span className="text-ink/70">#symbol</span>.
            </div>
          </div>

          <div>
            <Label>Edit targets</Label>
            <div className="flex flex-col gap-2">
              {targets.map((t, i) => (
                <div key={i} className="flex items-center gap-1.5">
                  <input
                    value={t}
                    onChange={(e) =>
                      setTargets((prev) => prev.map((x, j) => (j === i ? e.target.value : x)))
                    }
                    placeholder="src/schema.rs#Schema"
                    className={`${inputCls} font-term`}
                  />
                  {targets.length > 1 && (
                    <button
                      onClick={() => setTargets((prev) => prev.filter((_, j) => j !== i))}
                      aria-label="Remove target"
                      className={`grid place-items-center w-8 h-8 shrink-0 rounded-[4px] text-ink/50 hover:bg-flysch hover:text-ink ${focusRing}`}
                    >
                      <X size={14} />
                    </button>
                  )}
                </div>
              ))}
            </div>
            <button
              onClick={() => setTargets((prev) => [...prev, ""])}
              className={`${btn} mt-2 h-8 text-[12px]`}
            >
              <Plus size={13} />
              add target
            </button>
          </div>

          <div>
            <Label>Message (optional)</Label>
            <input
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              placeholder="Update the schema when the handler changes."
              className={inputCls}
            />
          </div>

          <div>
            <Label>Exclude (optional)</Label>
            <input
              value={exclude}
              onChange={(e) => setExclude(e.target.value)}
              placeholder="**/generated/**, **/*.test.rs"
              className={`${inputCls} font-term`}
            />
            <div className="mt-1 font-term text-[11px] text-ink/50">Comma-separated globs.</div>
          </div>

          <div className="text-[12px] text-ink/55 leading-[1.5]">
            Both endpoints must resolve in the repo.
          </div>
        </div>
      </Drawer>
    </Page>
  );
}
