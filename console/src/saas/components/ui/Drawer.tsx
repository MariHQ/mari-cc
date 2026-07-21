import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { focusRing } from "./focusRing";

/* ── Drawer: right-side detail slide-over ──────────────────────────────── */
const FOCUSABLE = 'a[href], button:not([disabled]), textarea, input, select, [tabindex]:not([tabindex="-1"])';

export function Drawer({ open, onClose, title, subtitle, icon, footer, children }: { open: boolean; onClose: () => void; title: string; subtitle?: string; icon?: ReactNode; footer?: ReactNode; children: ReactNode }) {
  const panelRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key !== "Tab") return;
      const panel = panelRef.current;
      if (!panel) return;
      const els = panel.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (els.length === 0) { e.preventDefault(); return; }
      const first = els[0];
      const last = els[els.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === panel)) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && active === last) { e.preventDefault(); first.focus(); }
    };

    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      previouslyFocused?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-[60] font-display">
      <div className="absolute inset-0 bg-ink/30 drawer-overlay" onClick={onClose} />
      <aside
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        className="absolute right-0 top-0 h-full w-full max-w-[460px] bg-paper border-l border-ink/15 shadow-2xl flex flex-col text-ink drawer-panel outline-none"
      >
        <header className="shrink-0 h-14 px-5 flex items-center gap-3 border-b border-ink/10">
          {icon}
          <div className="min-w-0 flex-1">
            <div className="text-[14px] font-semibold text-ink truncate">{title}</div>
            {subtitle && <div className="font-term text-[11px] uppercase tracking-[0.08em] text-ink/55 truncate">{subtitle}</div>}
          </div>
          <button onClick={onClose} className={`grid place-items-center w-8 h-8 rounded-[4px] text-ink/60 hover:bg-flysch hover:text-ink ${focusRing}`} aria-label="Close"><X size={16} /></button>
        </header>
        <div className="flex-1 overflow-y-auto p-5">{children}</div>
        {footer && <footer className="shrink-0 p-4 border-t border-ink/10 flex gap-2">{footer}</footer>}
      </aside>
    </div>
  );
}
