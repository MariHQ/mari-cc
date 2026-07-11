import { useEffect, useState } from "react";
import { ListChecks, Check, FileText } from "lucide-react";
import { api } from "@saas/lib/client";
import { Page, card, btn, btnPrimary } from "../console-ui";

type FactsData = { file: string; items: { claim: string }[]; raw: string };

export function FactsGroup() {
  const [data, setData] = useState<FactsData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [showRaw, setShowRaw] = useState(false);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .facts()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  const file = data?.file ?? "FACTS.md";

  const rawToggle = (
    <button
      onClick={() => setShowRaw((v) => !v)}
      className={showRaw ? btnPrimary : btn}
      aria-pressed={showRaw}
      title="Show the raw markdown of the facts file"
    >
      <FileText size={14} />
      Raw
    </button>
  );

  return (
    <Page
      title="Facts"
      subtitle={`The claims ledger factcheck grounds against — ${file}`}
      kicker="style"
      actions={data && !error ? rawToggle : undefined}
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load facts</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && showRaw && (
        <div className={`${card} mt-5 overflow-hidden`}>
          <div className="flex items-center gap-2 px-4 py-2.5 border-b border-ink/10">
            <FileText size={14} className="text-ink/50" />
            <span className="font-term text-[11.5px] text-ink/60">{file}</span>
          </div>
          <pre className="font-term text-[12px] leading-[1.6] text-ink/85 whitespace-pre-wrap break-words p-4 overflow-x-auto">
            {data.raw || "(empty)"}
          </pre>
        </div>
      )}

      {data && !error && !showRaw && data.items.length === 0 && (
        <div className={`${card} mt-5 p-6 text-center`}>
          <ListChecks size={22} className="mx-auto text-ink/25" />
          <div className="mt-2 text-[13px] text-ink/70">
            No facts recorded. Add claims to FACTS.md (one per line).
          </div>
          <div className="mt-1 font-term text-[11.5px] text-ink/45">{file}</div>
        </div>
      )}

      {data && !error && !showRaw && data.items.length > 0 && (
        <div className="mt-5 flex flex-col gap-2">
          <div className="flex items-center gap-2 font-term text-[11px] uppercase tracking-[0.08em] text-ink/55">
            <ListChecks size={14} className="text-biscay-2" />
            {data.items.length} claim{data.items.length === 1 ? "" : "s"}
          </div>
          {data.items.map((it, i) => (
            <div
              key={i}
              className={`${card} flex items-start gap-3 px-4 py-3`}
            >
              <span className="mt-[3px] grid place-items-center w-[18px] h-[18px] shrink-0 rounded-full border border-moss/40 bg-moss/[0.08]">
                <Check size={12} className="text-moss" />
              </span>
              <p className="font-display text-[13.5px] leading-[1.5] text-ink/90">{it.claim}</p>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}
