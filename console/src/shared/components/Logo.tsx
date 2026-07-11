// Brand mark + wordmark, matched to the landing page (redesign/components/ui.tsx).
export const Logo = ({ className = "" }: { className?: string }) => (
  <div className={`flex items-center gap-2 font-display font-extrabold tracking-[-0.04em] text-[19px] text-ink leading-none ${className}`}>
    <svg viewBox="0 0 32 32" fill="none" stroke="#10263B" strokeWidth={1.6} className="w-[24px] h-[24px]" aria-hidden>
      <path d="M4 27 V7 L16 18 L28 7 V27" />
      <rect x="2.2" y="5.2" width="3.6" height="3.6" fill="#10263B" />
      <rect x="14.2" y="16.2" width="3.6" height="3.6" fill="#1C3F60" stroke="#1C3F60" />
      <rect x="26.2" y="5.2" width="3.6" height="3.6" fill="#10263B" />
      <rect x="2.2" y="25.2" width="3.6" height="3.6" fill="#10263B" />
      <rect x="26.2" y="25.2" width="3.6" height="3.6" fill="#10263B" />
    </svg>
    <span>mari</span>
  </div>
);
