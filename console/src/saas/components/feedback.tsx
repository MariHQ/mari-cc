import { useEffect, useState } from "react";
import { CheckCircle2, AlertTriangle, Info } from "lucide-react";

export type ToastTone = "default" | "success" | "error";
type Toast = { id: number; message: string; tone: ToastTone };

let toasts: Toast[] = [];
let listeners: Array<(t: Toast[]) => void> = [];
let seq = 0;
const emit = () => listeners.forEach((l) => l(toasts));

/** Fire a toast from anywhere: `toast("Saved", "success")`. */
export function toast(message: string, tone: ToastTone = "default") {
  const t: Toast = { id: ++seq, message, tone };
  toasts = [...toasts, t];
  emit();
  setTimeout(() => { toasts = toasts.filter((x) => x.id !== t.id); emit(); }, 3200);
}

const ICON = { default: Info, success: CheckCircle2, error: AlertTriangle };
const COLOR = { default: "text-biscay-2", success: "text-moss", error: "text-espelette" };

export function Toaster() {
  const [items, setItems] = useState<Toast[]>(toasts);
  useEffect(() => {
    listeners.push(setItems);
    return () => { listeners = listeners.filter((l) => l !== setItems); };
  }, []);
  return (
    <div className="fixed bottom-4 right-4 z-[80] flex flex-col gap-2 font-display">
      {items.map((t) => {
        const Icon = ICON[t.tone];
        return (
          <div key={t.id} className="flex items-center gap-2.5 rounded-[4px] border border-ink/15 bg-paper shadow-lg px-3.5 py-2.5 min-w-[240px] toast-in">
            <Icon size={16} className={COLOR[t.tone]} />
            <span className="text-[13px] text-ink/90 flex-1">{t.message}</span>
          </div>
        );
      })}
    </div>
  );
}
