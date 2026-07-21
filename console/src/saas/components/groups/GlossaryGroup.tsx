import { useEffect, useState } from "react";
import { BookMarked } from "lucide-react";
import { api } from "@saas/lib/client";
import { Page, Table, Badge, card } from "../ui";

type GlossaryData = { file: string; terms: { use: string; variants: string[] }[] };

export function GlossaryGroup() {
  const [data, setData] = useState<GlossaryData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .glossary()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  const file = data?.file ?? "STYLE.md";

  return (
    <Page title="Glossary" subtitle={`Preferred terms and their variants — ${file}`} kicker="curation">
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 border-espelette/30 bg-espelette/[0.05] p-4`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load glossary</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && !error && data.terms.length === 0 && (
        <div className={`${card} mt-5 p-6 text-center`}>
          <BookMarked size={22} className="mx-auto text-ink/25" />
          <div className="mx-auto mt-2 max-w-[440px] text-[13px] text-ink/70">
            No glossary terms yet. Add a Terminology table (a <span className="font-term">| Use | Not |</span> markdown
            table) to {file}, or run <span className="font-term">/mari</span> to harvest terms with an agent.
          </div>
          <div className="mt-1 font-term text-[11.5px] text-ink/45">{file}</div>
        </div>
      )}

      {data && !error && data.terms.length > 0 && (
        <Table title="Terminology" count={data.terms.length} head={["Use", "Instead of"]} minW={560}>
          {data.terms.map((t, i) => (
            <tr key={`${t.use}-${i}`} className="border-b border-ink/10 align-top last:border-0">
              <td className="whitespace-nowrap px-4 py-3 text-[13px] font-medium text-ink">{t.use}</td>
              <td className="px-4 py-3">
                {t.variants.length === 0 ? (
                  <span className="font-term text-[12px] text-ink/40">—</span>
                ) : (
                  <div className="flex flex-wrap gap-1.5">
                    {t.variants.map((v, j) => (
                      <Badge key={`${v}-${j}`} label={v} tone="neutral" />
                    ))}
                  </div>
                )}
              </td>
            </tr>
          ))}
        </Table>
      )}
    </Page>
  );
}
