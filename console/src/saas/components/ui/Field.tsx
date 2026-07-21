import type { ReactNode } from "react";

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="py-2.5 border-b border-ink/10 last:border-0">
      <div className="font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55">{label}</div>
      <div className="mt-1 text-[13px] text-ink/90">{children}</div>
    </div>
  );
}
