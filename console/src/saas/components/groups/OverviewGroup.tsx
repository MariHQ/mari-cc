import { Page, card } from "../console-ui";

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className={`${card} px-5 py-4`}>
      <div className="font-term text-[30px] font-semibold leading-none text-biscay">{value}</div>
      <div className="mt-2 text-[12.5px] text-ink/60">{label}</div>
    </div>
  );
}

export function OverviewGroup() {
  return (
    <Page title="Overview" subtitle="Deterministic prose checks and repository style controls." kicker="mari">
      <div className="mt-5 grid gap-3 sm:grid-cols-3">
        <Stat value="170+" label="prose rules" />
        <Stat value="49" label="configurable word lists" />
        <Stat value="5" label="built-in style guides" />
      </div>
      <div className={`${card} mt-5 p-5`}>
        <h3 className="text-[15px] font-semibold text-ink">Repository-local by design</h3>
        <p className="mt-2 max-w-2xl text-[13px] leading-6 text-ink/65">
          Settings live in <span className="font-term text-ink/85">.mari/config.json</span>. Use the Detector,
          Rules, Glossary, and Localization panels to manage the writing system for this repository.
        </p>
      </div>
    </Page>
  );
}
