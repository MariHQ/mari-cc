import { useEffect, useState } from "react";
import { Globe, Check, X, ArrowRight } from "lucide-react";
import { api, type DocsiteInfo } from "@saas/lib/client";
import { Page, Badge, card } from "../console-ui";

const CHECKS: { key: keyof DocsiteInfo["status"]; label: string }[] = [
  { key: "docs_dir", label: "docs/ directory" },
  { key: "readme", label: "README" },
  { key: "license", label: "LICENSE" },
  { key: "contributing", label: "CONTRIBUTING" },
  { key: "code_of_conduct", label: "CODE_OF_CONDUCT" },
  { key: "security", label: "SECURITY" },
  { key: "changelog", label: "CHANGELOG" },
];

export function DocsiteGroup() {
  const [data, setData] = useState<DocsiteInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .docsite()
      .then((d) => setData(d))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  const s = data?.status;
  const present = s ? CHECKS.filter((c) => s[c.key]).length : 0;

  return (
    <Page
      title="Docsite"
      subtitle="Docs-site readiness and the build/keep-alive plan. The commands run in your terminal or via /mari."
      kicker="docs"
    >
      {loading && !data && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className={`${card} mt-5 p-4 border-espelette/30 bg-espelette/[0.05]`}>
          <div className="text-[13px] font-medium text-espelette">Couldn’t load docsite status</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">
            Retry
          </button>
        </div>
      )}

      {data && s && !error && (
        <>
          {/* Readiness */}
          <div className="mt-5 grid gap-5 lg:grid-cols-2 items-start">
            <div className={`${card} p-4`}>
              <div className="flex items-center gap-2">
                <Globe size={15} className="text-biscay-2" />
                <h4 className="text-[14px] font-semibold text-ink">Readiness</h4>
                <span className="ml-auto font-term text-[11px] text-ink/55">
                  {present}/{CHECKS.length} files
                </span>
              </div>

              <div className="mt-3 flex items-center gap-2">
                <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">platform</span>
                <Badge label={s.platform ?? "none detected"} tone={s.platform ? "info" : "neutral"} />
              </div>

              <ul className="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-x-4">
                {CHECKS.map((c) => (
                  <li key={c.key} className="flex items-center gap-2 py-1.5 border-b border-ink/10 last:border-0">
                    {s[c.key] ? (
                      <Check size={15} className="text-moss shrink-0" />
                    ) : (
                      <X size={15} className="text-espelette shrink-0" />
                    )}
                    <span className={`text-[13px] ${s[c.key] ? "text-ink/85" : "text-ink/50"}`}>{c.label}</span>
                  </li>
                ))}
              </ul>

              <div className="mt-3 flex items-center gap-4 border-t border-ink/10 pt-3">
                <div className="flex items-center gap-1.5">
                  <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">hook</span>
                  <Badge label={s.hook_configured ? "on" : "off"} tone={s.hook_configured ? "ok" : "neutral"} />
                </div>
                <div className="flex items-center gap-1.5">
                  <span className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/50">edit-notify rules</span>
                  <Badge label={s.rules_configured ? "configured" : "none"} tone={s.rules_configured ? "ok" : "neutral"} />
                </div>
              </div>

              <div className="mt-1 font-term text-[11px] text-ink/45 break-all pt-2">{s.root}</div>
            </div>

            {/* Next commands */}
            <div className={`${card} p-4`}>
              <div className="flex items-center gap-2">
                <ArrowRight size={15} className="text-biscay-2" />
                <h4 className="text-[14px] font-semibold text-ink">Recommended next</h4>
              </div>
              {s.next_commands.length === 0 ? (
                <div className="mt-3 text-[13px] text-ink/60">Nothing outstanding — the docs scaffold looks complete.</div>
              ) : (
                <ul className="mt-3 flex flex-col gap-2">
                  {s.next_commands.map((cmd, i) => (
                    <li
                      key={i}
                      className="font-term text-[12.5px] text-ink/85 bg-flysch/70 border border-ink/10 rounded-[4px] px-2.5 py-1.5"
                    >
                      {cmd}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>

          {/* Build plan */}
          <div className={`${card} mt-5 overflow-hidden`}>
            <div className="flex items-center gap-2 px-4 py-3 border-b border-ink/10 bg-flysch/40">
              <Globe size={15} className="text-biscay-2" />
              <h4 className="text-[14px] font-semibold text-ink">Build &amp; keep-alive plan</h4>
              <span className="font-term text-[11px] font-medium text-ink/55 bg-paper border border-ink/10 rounded-[3px] px-1.5 py-0.5">
                {data.plan.phases.length} phases
              </span>
            </div>
            <ol>
              {data.plan.phases.map((p, i) => (
                <li key={i} className="flex gap-3 px-4 py-3 border-b border-ink/10 last:border-0">
                  <span className="grid place-items-center w-6 h-6 shrink-0 rounded-full bg-biscay/[0.08] border border-biscay-2/30 font-term text-[11px] font-semibold text-biscay-2">
                    {i + 1}
                  </span>
                  <div className="min-w-0">
                    <div className="text-[13px] font-medium text-ink">{p.phase}</div>
                    <code className="inline-block mt-0.5 font-term text-[12px] text-biscay-2 bg-biscay/[0.06] rounded-[3px] px-1.5 py-0.5">
                      {p.command}
                    </code>
                    <div className="mt-1 text-[12.5px] text-ink/60">{p.output}</div>
                  </div>
                </li>
              ))}
            </ol>
          </div>
        </>
      )}
    </Page>
  );
}
