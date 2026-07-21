import type { ReactNode } from "react";
import { card } from "./card";

const thClass = "font-term font-medium text-[11px] uppercase tracking-[0.08em] text-ink/60";

export function Table({ title, count, head, footer, minW = 700, children }: { title?: string; count?: number; head: string[]; footer?: ReactNode; minW?: number; children: ReactNode }) {
  return (
    <div className={`${card} mt-5 overflow-hidden`}>
      {title && (
        <div className="flex items-center gap-2 px-4 pt-4 pb-3">
          <h4 className="text-[15px] font-semibold text-ink">{title}</h4>
          {count != null && <span className="font-term text-[11px] font-medium text-ink/60 bg-flysch border border-ink/10 rounded-[3px] px-1.5 py-0.5">{count}</span>}
        </div>
      )}
      <div className="overflow-x-auto">
        <table className="w-full text-left border-collapse" style={{ minWidth: minW }}>
          <thead>
            <tr>
              {head.map((h) => <th key={h} className={`${thClass} px-4 py-2.5 border-y border-ink/10`}>{h}</th>)}
            </tr>
          </thead>
          <tbody>{children}</tbody>
        </table>
      </div>
      {footer}
    </div>
  );
}
