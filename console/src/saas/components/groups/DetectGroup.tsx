import { useState } from "react";
import { SpellCheck, Play, Ban, EyeOff, FileText, Type } from "lucide-react";
import { api, type DetectResult, type DetectFinding } from "@saas/lib/client";
import { Page, Badge, card, btn, btnPrimary, focusRing } from "../console-ui";
import { toast } from "../feedback";

/* Detector severity → badge tone. */
function sevTone(sev: string): string {
  const s = sev.toLowerCase();
  if (s === "error" || s === "high") return "blocked";
  if (s === "warn" || s === "warning") return "attention";
  if (s === "advisory" || s === "low") return "neutral";
  return "info";
}

/* Slop score band → color for the gauge number. */
function bandColor(band: string): string {
  const b = band.toLowerCase();
  if (b.includes("heavy") || b.includes("high")) return "text-espelette";
  if (b.includes("some") || b.includes("moderate")) return "text-clay";
  return "text-moss";
}

const SAMPLE =
  "In today’s fast-paced world, we leverage cutting-edge synergies to seamlessly delve into robust solutions. It is important to note that this is a very unique, game-changing paradigm.";

export function DetectGroup() {
  const [mode, setMode] = useState<"text" | "file">("text");
  const [text, setText] = useState("");
  const [path, setPath] = useState("");
  const [result, setResult] = useState<DetectResult | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function analyze() {
    setRunning(true);
    setError(null);
    try {
      const input = mode === "file" ? { path: path.trim() } : { text };
      if (mode === "file" && !path.trim()) {
        setError("Enter a repo-relative file path.");
        return;
      }
      if (mode === "text" && !text.trim()) {
        setError("Paste some text to analyze.");
        return;
      }
      const r = await api.detect(input);
      setResult(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
    }
  }

  async function waive(f: DetectFinding, kind: "ignore" | "zero") {
    try {
      if (kind === "ignore") await api.setIgnore(f.ruleId, "add");
      else await api.setZero(f.ruleId, "add");
      toast(kind === "ignore" ? `Ignoring “${f.ruleId}”` : `“${f.ruleId}” set zero-tolerance`, "success");
      // Re-run so the change is reflected (ignored rules drop out).
      await analyze();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    }
  }

  const s = result?.score;

  return (
    <Page
      title="Detector"
      subtitle="Run the deterministic prose detector on text or a repo file — findings, slop score, and one-click waivers."
      kicker="governance"
    >
      {/* Input */}
      <div className={`${card} mt-5 p-4`}>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setMode("text")}
            className={`inline-flex items-center gap-1.5 h-8 px-2.5 rounded-[4px] text-[12.5px] font-medium ${focusRing} ${
              mode === "text" ? "bg-biscay text-white" : "border border-ink/20 text-ink/70 hover:text-ink"
            }`}
          >
            <Type size={13} /> Paste text
          </button>
          <button
            onClick={() => setMode("file")}
            className={`inline-flex items-center gap-1.5 h-8 px-2.5 rounded-[4px] text-[12.5px] font-medium ${focusRing} ${
              mode === "file" ? "bg-biscay text-white" : "border border-ink/20 text-ink/70 hover:text-ink"
            }`}
          >
            <FileText size={13} /> Repo file
          </button>
          {mode === "text" && (
            <button onClick={() => setText(SAMPLE)} className="ml-auto font-term text-[11.5px] text-biscay-2 hover:underline">
              try a sample
            </button>
          )}
        </div>

        {mode === "text" ? (
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Paste prose to check for AI slop, clarity, and house-style issues…"
            rows={7}
            spellCheck={false}
            className={`mt-3 w-full px-3 py-2.5 rounded-[4px] border border-ink/20 bg-paper text-[13px] leading-[1.6] text-ink outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 resize-y ${focusRing}`}
          />
        ) : (
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && analyze()}
            placeholder="docs/setup.md"
            className={`mt-3 w-full h-9 px-3 rounded-[4px] border border-ink/20 bg-paper font-term text-[13px] text-ink outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`}
          />
        )}

        <div className="mt-3 flex items-center gap-3">
          <button onClick={analyze} disabled={running} className={`${btnPrimary} disabled:opacity-60`}>
            <Play size={14} />
            {running ? "Analyzing…" : "Analyze"}
          </button>
          {error && <span className="font-term text-[12px] text-espelette">{error}</span>}
        </div>
      </div>

      {/* Results */}
      {result && s && (
        <>
          <div className="mt-5 grid gap-4 sm:grid-cols-[auto_1fr] items-stretch">
            {/* Score */}
            <div className={`${card} px-5 py-4 flex flex-col justify-center min-w-[150px]`}>
              <div className="font-term text-[10px] uppercase tracking-[0.12em] text-ink/50">Slop score</div>
              <div className={`font-term text-[40px] font-bold leading-none mt-1 ${bandColor(s.band)}`}>{s.score}</div>
              <div className="font-term text-[11.5px] text-ink/60 mt-1">
                {s.band} · {s.words} words · {s.per1k.toFixed(1)}/1k
              </div>
            </div>
            {/* By family */}
            <div className={`${card} px-4 py-3`}>
              <div className="font-term text-[10px] uppercase tracking-[0.12em] text-ink/50 mb-2">
                {s.findingCount} finding{s.findingCount === 1 ? "" : "s"} · style guide {result.styleGuide}
              </div>
              {Object.keys(s.byFamily).length === 0 ? (
                <div className="text-[13px] text-moss">Clean — no findings.</div>
              ) : (
                <div className="flex flex-col gap-1.5">
                  {Object.entries(s.byFamily)
                    .sort((a, b) => b[1] - a[1])
                    .map(([fam, n]) => {
                      const max = Math.max(...Object.values(s.byFamily));
                      return (
                        <div key={fam} className="flex items-center gap-2">
                          <span className="font-term text-[11.5px] text-ink/70 w-[110px] shrink-0">{fam}</span>
                          <div className="flex-1 h-2.5 bg-flysch rounded-[2px] overflow-hidden">
                            <div className="h-full bg-biscay-2" style={{ width: `${(n / max) * 100}%` }} />
                          </div>
                          <span className="font-term text-[11.5px] text-ink/70 w-6 text-right">{n}</span>
                        </div>
                      );
                    })}
                </div>
              )}
            </div>
          </div>

          {/* Findings */}
          {result.findings.length > 0 && (
            <div className={`${card} mt-5 overflow-hidden`}>
              <div className="flex items-center gap-2 px-4 py-3 border-b border-ink/10">
                <SpellCheck size={15} className="text-biscay-2" />
                <h4 className="text-[14px] font-semibold text-ink">Findings</h4>
                <span className="font-term text-[11px] font-medium text-ink/55 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5">
                  {result.findings.length}
                </span>
              </div>
              <div className="max-h-[560px] overflow-y-auto">
                {result.findings.map((f, i) => (
                  <div key={i} className="flex items-start gap-3 px-4 py-2.5 border-b border-ink/10 last:border-0">
                    <span className="font-term text-[11px] text-ink/40 w-10 shrink-0 pt-0.5">L{f.line}</span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-term text-[12px] text-ink/90">{f.ruleId}</span>
                        <Badge label={f.family} tone="neutral" />
                        <Badge label={f.severity} tone={sevTone(f.severity)} />
                      </div>
                      <div className="mt-0.5 text-[13px] text-ink/80">{f.message}</div>
                      {f.span && (
                        <div className="mt-1 font-term text-[12px] text-ink/70">
                          <span className="bg-clay/[0.12] text-clay rounded-[2px] px-1 py-0.5">{f.span}</span>
                        </div>
                      )}
                    </div>
                    <div className="flex items-center gap-1.5 shrink-0">
                      <button
                        onClick={() => waive(f, "ignore")}
                        title="Waive this rule (add to detector.ignoreRules)"
                        className={`${btn} h-7 px-2 text-[12px]`}
                      >
                        <EyeOff size={12} /> Ignore
                      </button>
                      <button
                        onClick={() => waive(f, "zero")}
                        title="Escalate this rule to zero-tolerance"
                        className={`${btn} h-7 px-2 text-[12px]`}
                      >
                        <Ban size={12} /> Zero
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
          {result.findings.length === 0 && (
            <div className={`${card} mt-5 p-6 text-center text-[13px] text-moss`}>
              Clean — the detector found nothing to flag.
            </div>
          )}
        </>
      )}
    </Page>
  );
}
