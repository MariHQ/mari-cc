import type { ReactNode } from "react";

export function Page({ title, subtitle, kicker, actions, children }: { title: string; subtitle: string; kicker?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <div className="font-display text-ink bg-paper min-h-full p-4 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          {kicker && (
            <div className="flex items-center gap-2 mb-1.5 font-term text-[10.5px] font-medium uppercase tracking-[0.18em] text-biscay-2">
              <span className="inline-block w-[7px] h-[7px] bg-biscay-2" aria-hidden />
              {kicker}
            </div>
          )}
          <h3 className="text-[22px] font-bold tracking-[-0.015em] text-ink">{title}</h3>
          <p className="text-[13px] text-ink/60 mt-1 max-w-[680px]">{subtitle}</p>
        </div>
        {actions && <div className="flex items-center gap-2">{actions}</div>}
      </div>
      {children}
    </div>
  );
}
