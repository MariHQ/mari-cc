import { useEffect, useState } from "react";
import { FileStack, FilePlus2 } from "lucide-react";
import { api, type Template } from "@saas/lib/client";
import { toast } from "../feedback";
import { Page, Badge, card, btnPrimary, btnDanger, focusRing } from "../ui";

function TemplateCard({ tpl, onDone }: { tpl: Template; onDone: () => void }) {
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmForce, setConfirmForce] = useState(false);

  async function scaffold(force: boolean) {
    setBusy(true);
    try {
      await api.scaffoldTemplate(tpl.id, title.trim() || undefined, force || undefined);
      toast(`Created ${tpl.file}`, "success");
      setConfirmForce(false);
      setTitle("");
      onDone();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (!force && /exist/i.test(msg)) {
        setConfirmForce(true);
      } else {
        toast(msg, "error");
        setConfirmForce(false);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={`${card} flex flex-col p-4`}>
      <div className="text-[15px] font-semibold text-ink">{tpl.title}</div>
      <div className="mt-0.5 font-term text-[12px] text-biscay-2">{tpl.file}</div>
      {tpl.basis && (
        <div className="mt-1 text-[12px] text-ink/50">based on: {tpl.basis}</div>
      )}

      {tpl.sections.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {tpl.sections.map((s, i) => (
            <Badge key={`${s}-${i}`} label={s} tone="neutral" />
          ))}
        </div>
      )}

      <div className="mt-4 flex items-center gap-2 border-t border-ink/10 pt-3">
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Optional title"
          className={`h-9 min-w-0 flex-1 rounded-[4px] border border-ink/20 bg-paper px-2.5 text-[13px] text-ink placeholder:text-ink/40 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`}
        />
        {confirmForce ? (
          <button
            onClick={() => scaffold(true)}
            disabled={busy}
            className={`${btnDanger} shrink-0 disabled:opacity-50`}
          >
            Overwrite?
          </button>
        ) : (
          <button
            onClick={() => scaffold(false)}
            disabled={busy}
            className={`${btnPrimary} shrink-0 disabled:opacity-50`}
          >
            <FilePlus2 size={14} />
            Scaffold
          </button>
        )}
      </div>
      {confirmForce && (
        <div className="mt-2 font-term text-[11.5px] text-espelette">
          {tpl.file} already exists. Overwrite it?
        </div>
      )}
    </div>
  );
}

export function TemplatesGroup() {
  const [templates, setTemplates] = useState<Template[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .templates()
      .then((d) => setTemplates(d.templates))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  return (
    <Page
      title="Templates"
      subtitle="Document archetypes — scaffold a new doc from a standard template."
      kicker="curation"
    >
      {loading && !templates && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 border-espelette/30 bg-espelette/[0.05] p-4`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load templates</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {templates && !error && templates.length === 0 && (
        <div className={`${card} mt-5 p-6 text-center`}>
          <FileStack size={22} className="mx-auto text-ink/25" />
          <div className="mt-2 text-[13px] text-ink/70">No templates available.</div>
        </div>
      )}

      {templates && !error && templates.length > 0 && (
        <div className="mt-5 grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {templates.map((tpl) => (
            <TemplateCard key={tpl.id} tpl={tpl} onDone={reload} />
          ))}
        </div>
      )}
    </Page>
  );
}
