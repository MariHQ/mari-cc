import { focusRing } from "./focusRing";

export const btn = `inline-flex items-center gap-1.5 h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] font-medium text-ink/80 hover:border-ink/45 hover:text-ink transition-colors ${focusRing}`;
export const btnPrimary = `inline-flex items-center gap-1.5 h-9 px-3.5 rounded-[4px] bg-biscay text-white text-[13px] font-semibold hover:bg-biscay-2 transition-colors ${focusRing}`;
export const btnDanger = `inline-flex items-center gap-1.5 h-9 px-3 rounded-[4px] border border-espelette/40 bg-paper text-[13px] font-medium text-espelette hover:bg-espelette/[0.06] hover:border-espelette transition-colors ${focusRing}`;
