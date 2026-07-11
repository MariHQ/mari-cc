import { useEffect, useMemo, useState } from "react";
import { Search, CornerDownLeft } from "lucide-react";
import { STEPS, type StepId } from "@saas/lib/pipeline";

export function CommandPalette({ open, onClose, onNavigate }: { open: boolean; onClose: () => void; onNavigate: (id: StepId) => void }) {
  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);

  const results = useMemo(
    () => STEPS.filter((s) => s.name.toLowerCase().includes(q.toLowerCase()) || s.description.toLowerCase().includes(q.toLowerCase())),
    [q],
  );

  useEffect(() => { setIdx(0); }, [q, open]);
  useEffect(() => { if (!open) setQ(""); }, [open]);

  if (!open) return null;

  const choose = (i: number) => { const r = results[i]; if (r) { onNavigate(r.id); onClose(); } };

  return (
    <div
      className="fixed inset-0 z-[70] font-display"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        else if (e.key === "ArrowDown") { e.preventDefault(); setIdx((i) => Math.min(results.length - 1, i + 1)); }
        else if (e.key === "ArrowUp") { e.preventDefault(); setIdx((i) => Math.max(0, i - 1)); }
        else if (e.key === "Enter") { e.preventDefault(); choose(idx); }
      }}
    >
      <div className="absolute inset-0 bg-ink/30" onClick={onClose} />
      <div className="absolute left-1/2 top-[14vh] -translate-x-1/2 w-full max-w-[560px] px-4">
        <div className="rounded-md border border-ink/15 bg-paper shadow-2xl overflow-hidden cmd-in">
          <div className="flex items-center gap-2.5 px-4 h-12 border-b border-ink/10">
            <Search size={16} className="text-ink/50" />
            <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Jump to a section…" className="flex-1 bg-transparent text-[14px] text-ink placeholder:text-ink/45 outline-none" />
            <kbd className="font-term text-[10px] text-ink/55 border border-ink/20 rounded-[3px] px-1.5 py-0.5">ESC</kbd>
          </div>
          <div className="max-h-[320px] overflow-y-auto p-2">
            {results.length === 0 ? (
              <div className="px-3 py-8 text-center text-[13px] text-ink/60">No matches</div>
            ) : (
              results.map((r, i) => {
                const Icon = r.icon;
                return (
                  <button
                    key={r.id}
                    onMouseEnter={() => setIdx(i)}
                    onClick={() => choose(i)}
                    className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-[4px] text-left ${i === idx ? "bg-biscay-2/10" : "hover:bg-flysch/70"}`}
                  >
                    <Icon size={15} className={i === idx ? "text-biscay-2" : "text-ink/50"} />
                    <span className="flex-1 min-w-0">
                      <span className={`text-[13px] font-medium ${i === idx ? "text-biscay-2" : "text-ink"}`}>{r.name}</span>
                      <span className="block text-[11.5px] text-ink/55 truncate">{r.description}</span>
                    </span>
                    {i === idx && <CornerDownLeft size={13} className="text-biscay-2 shrink-0" />}
                  </button>
                );
              })
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
