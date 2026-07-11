import type { Step } from "@saas/lib/pipeline";

export function Placeholder({ step }: { step: Step }) {
  const Icon = step.icon;
  return (
    <div className="font-display text-ink bg-paper min-h-full p-4 sm:p-6">
      <div className="flex items-center gap-2.5">
        <span className="inline-flex items-center justify-center w-9 h-9 rounded-[4px] bg-biscay-2/10 text-biscay-2"><Icon size={18} /></span>
        <h3 className="text-[21px] font-bold tracking-[-0.02em] text-ink">{step.name}</h3>
      </div>
      <p className="text-[14px] text-ink/60 mt-3 leading-relaxed max-w-[640px]">{step.description}</p>
      <div className="mt-6 rounded-md border border-dashed border-ink/25 bg-paper px-5 py-10 text-center text-[13px] text-ink/55">
        This section is coming soon.
      </div>
    </div>
  );
}
