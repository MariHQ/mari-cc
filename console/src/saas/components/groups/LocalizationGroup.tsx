import { useEffect, useMemo, useState } from "react";
import { Languages, Check } from "lucide-react";
import { api, type Localization } from "@saas/lib/client";
import { Page, Badge, card } from "../console-ui";

const shortName = (p: string) => p.split("/").pop() || p;

export function LocalizationGroup() {
  const [data, setData] = useState<Localization | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .localization()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  const stats = useMemo(() => {
    if (!data) return { localized: 0, stale: 0, cells: 0 };
    let stale = 0;
    let cells = 0;
    for (const s of data.sources) {
      for (const lang of Object.keys(s.translations)) {
        cells++;
        if (s.translations[lang].stale) stale++;
      }
    }
    return { localized: data.sources.length, stale, cells };
  }, [data]);

  return (
    <Page
      title="Localization"
      subtitle="Translation coverage across languages — read-only. Run `mari i18n coverage` / `conform` for deep checks."
      kicker="docs"
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load localization</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && data.languages.length === 0 && (
        <div className={`${card} mt-5 p-6 text-center`}>
          <Languages size={22} className="mx-auto text-ink/25" />
          <div className="mx-auto mt-2 max-w-[460px] text-[13px] text-ink/70">
            No translations found. Mari detects <span className="font-term">.es.md</span>-style suffixes,{" "}
            <span className="font-term">/es/</span> language directories, and Hugo/Docusaurus layouts. Add a
            translated sibling of a doc and it appears here.
          </div>
          <div className="mt-1 font-term text-[11.5px] text-ink/45">
            source languages: {data.sourceLangs.join(", ")}
          </div>
        </div>
      )}

      {data && !error && data.languages.length > 0 && (
        <>
          <div className="mt-5 grid grid-cols-3 gap-4 max-w-[560px]">
            <Stat label="Languages" value={data.languages.length} />
            <Stat label="Localized docs" value={stats.localized} />
            <Stat label="Stale" value={stats.stale} tone={stats.stale > 0 ? "warn" : "ok"} />
          </div>

          <div className={`${card} mt-5 overflow-hidden`}>
            <div className="flex items-center gap-2 px-4 py-3 border-b border-ink/10">
              <Languages size={15} className="text-biscay-2" />
              <h4 className="text-[14px] font-semibold text-ink">Translation matrix</h4>
              <span className="font-term text-[11px] font-medium text-ink/55 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5">
                {data.sources.length}
              </span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse" style={{ minWidth: 480 + data.languages.length * 90 }}>
                <thead>
                  <tr>
                    <th className="font-term font-medium text-[11px] uppercase tracking-[0.08em] text-ink/60 px-4 py-2.5 border-b border-ink/10">
                      Source doc
                    </th>
                    {data.languages.map((l) => (
                      <th
                        key={l}
                        className="font-term font-medium text-[11px] uppercase tracking-[0.08em] text-ink/60 px-3 py-2.5 border-b border-ink/10 text-center"
                      >
                        {l}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {data.sources.map((s) => (
                    <tr key={s.source} className="border-b border-ink/10 last:border-0 hover:bg-flysch/40">
                      <td className="px-4 py-2.5">
                        <div className="text-[13px] text-ink font-medium truncate max-w-[320px]" title={s.source}>
                          {shortName(s.source)}
                        </div>
                        <div className="font-term text-[11px] text-ink/45 truncate max-w-[320px]">{s.source}</div>
                      </td>
                      {data.languages.map((l) => {
                        const cell = s.translations[l];
                        return (
                          <td key={l} className="px-3 py-2.5 text-center">
                            {!cell ? (
                              <span className="text-ink/25">—</span>
                            ) : cell.stale ? (
                              <span title={`${cell.path} (stale — source is newer)`}>
                                <Badge label="stale" tone="attention" />
                              </span>
                            ) : (
                              <span title={cell.path} className="inline-flex text-moss">
                                <Check size={16} />
                              </span>
                            )}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
          <p className="mt-3 font-term text-[11.5px] text-ink/45">
            ✓ = translation present · <span className="text-clay">stale</span> = source changed after the
            translation · — = missing. Staleness compares file modified times.
          </p>
        </>
      )}
    </Page>
  );
}

function Stat({ label, value, tone }: { label: string; value: number; tone?: "ok" | "warn" }) {
  const color = tone === "warn" ? "text-clay" : tone === "ok" ? "text-moss" : "text-ink";
  return (
    <div className={`${card} px-4 py-3`}>
      <div className="font-term text-[10px] uppercase tracking-[0.12em] text-ink/50">{label}</div>
      <div className={`mt-1 font-term text-[24px] font-semibold ${color}`}>{value}</div>
    </div>
  );
}
