import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ChevronRight, Menu, X, Search } from "lucide-react";
import { Logo } from "@/components/Logo";
import { STEPS, stepById, type StepId, type Step } from "@saas/lib/pipeline";
import { focusRing } from "./ui";

import { OverviewGroup } from "./groups/OverviewGroup";
import { GlossaryGroup } from "./groups/GlossaryGroup";
import { TemplatesGroup } from "./groups/TemplatesGroup";
import { LocalizationGroup } from "./groups/LocalizationGroup";
import { DetectGroup } from "./groups/DetectGroup";
import { RulesGroup } from "./groups/RulesGroup";
import { NudgesGroup } from "./groups/NudgesGroup";
import { ConfigGroup } from "./groups/ConfigGroup";
import { RenderCrashBoundary } from "./RenderCrashBoundary";
import { CommandPalette } from "./CommandPalette";
import { Toaster } from "./feedback";

const VALID_IDS = new Set<string>(STEPS.map((s) => s.id));
const DEFAULT_STEP: StepId = "overview";

const renderGroup = (step: Step) => {
  switch (step.id) {
    case "overview":  return <OverviewGroup />;
    case "glossary":  return <GlossaryGroup />;
    case "templates": return <TemplatesGroup />;
    case "localization": return <LocalizationGroup />;
    case "detect":    return <DetectGroup />;
    case "rules":     return <RulesGroup />;
    case "nudges":    return <NudgesGroup />;
    case "config":    return <ConfigGroup />;
    default:          return <OverviewGroup />;
  }
};

export const Console = () => {
  const navigate = useNavigate();
  const params = useParams<{ stepId?: string }>();
  const urlStep = (params.stepId && VALID_IDS.has(params.stepId) ? params.stepId : DEFAULT_STEP) as StepId;

  const [activeId, setActiveId] = useState<StepId>(urlStep);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); setPaletteOpen((o) => !o); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (urlStep !== activeId) setActiveId(urlStep);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [urlStep]);

  const goTo = (id: StepId) => {
    setActiveId(id);
    navigate(`/console/${id}`);
  };

  const active = stepById(activeId);

  const bottomSteps = STEPS.filter((s) => s.pinBottom);
  const navGroups: { label: string | null; items: Step[] }[] = [];
  for (const s of STEPS.filter((x) => !x.pinBottom)) {
    const label = s.group ?? null;
    const last = navGroups[navGroups.length - 1];
    if (last && last.label === label) last.items.push(s);
    else navGroups.push({ label, items: [s] });
  }

  const sidebarItem = (step: Step, onPick?: () => void) => {
    const isActive = step.id === activeId;
    return (
      <li key={step.id}>
        <button
          onClick={() => { goTo(step.id); onPick?.(); }}
          aria-current={isActive ? "page" : undefined}
          className={`group w-full text-left flex items-center gap-2.5 pl-3 pr-2 py-2 md:py-1.5 rounded-[4px] text-sm transition-colors relative ${focusRing}
            ${isActive
              ? "bg-biscay-2/[0.08] text-biscay font-semibold"
              : "text-ink/65 hover:text-ink hover:bg-flysch/70"}`}
        >
          {isActive && <span className="absolute left-0 top-1 bottom-1 w-[3px] bg-biscay-2" />}
          <step.icon className={`h-3.5 w-3.5 ${isActive ? "text-biscay-2" : ""}`} />
          <span className="flex-1 truncate">{step.name}</span>
          {!isActive && <ChevronRight className="h-3 w-3 opacity-0 md:group-hover:opacity-100 transition-opacity" />}
        </button>
      </li>
    );
  };

  const renderSidebarBody = (onPick?: () => void) => (
    <div className="flex-1 flex flex-col min-h-0">
      <nav className="flex-1 overflow-y-auto p-2">
        {navGroups.map((g, gi) => (
          <div key={gi} className={gi > 0 ? "mt-4" : ""}>
            {g.label && (
              <div className="flex items-center gap-1.5 px-3 pb-1.5 font-term text-[10px] font-medium uppercase tracking-[0.14em] text-ink/45">
                <span className="inline-block w-[5px] h-[5px] bg-ink/30" aria-hidden />
                {g.label}
              </div>
            )}
            <ul className="space-y-0.5">{g.items.map((s) => sidebarItem(s, onPick))}</ul>
          </div>
        ))}
      </nav>
      {bottomSteps.length > 0 && (
        <nav className="p-2 border-t border-ink/10">
          <ul className="space-y-0.5">{bottomSteps.map((s) => sidebarItem(s, onPick))}</ul>
        </nav>
      )}
    </div>
  );

  return (
    <div className="h-[100dvh] w-screen flex flex-col bg-paper text-ink overflow-hidden font-display">
      {/* HEADER */}
      <header className="h-12 shrink-0 border-b border-ink/15 bg-paper flex items-center px-2 sm:px-3 gap-1.5 sm:gap-2.5">
        <button
          onClick={() => setSidebarOpen(true)}
          className={`md:hidden text-ink/50 hover:text-ink p-2 -ml-1 rounded-[4px] hover:bg-flysch transition-colors ${focusRing}`}
          aria-label="Open menu"
        >
          <Menu className="h-4 w-4" />
        </button>
        <Logo />
        <span className="hidden sm:inline-flex items-center rounded-full bg-biscay-2/[0.08] px-2 py-1 font-term text-[10px] font-semibold uppercase tracking-[0.08em] text-biscay-2">
          49 word lists
        </span>
        <span className="hidden md:inline font-term text-[12px] text-ink/55 lowercase"><span className="text-ink/30 mr-1.5">/</span>{active.name}</span>

        <div className="flex-1" />

        <button onClick={() => setPaletteOpen(true)} className={`hidden sm:inline-flex items-center gap-2 h-8 pl-2.5 pr-2 rounded-[4px] border border-ink/20 bg-paper text-ink/55 hover:border-ink/45 hover:text-ink/80 transition-colors ${focusRing}`} aria-label="Jump to section">
          <Search className="h-3.5 w-3.5" />
          <span className="text-[12.5px]">Jump to…</span>
          <kbd className="font-term text-[10px] text-ink/55 border border-ink/20 rounded-[3px] px-1 py-0.5 ml-1">⌘K</kbd>
        </button>
      </header>

      {/* MOBILE SIDEBAR DRAWER */}
      {sidebarOpen && (
        <div className="md:hidden fixed inset-0 z-50">
          <div className="absolute inset-0 bg-ink/40" onClick={() => setSidebarOpen(false)} />
          <aside className="relative h-full w-72 max-w-[85vw] bg-paper border-r border-ink/15 flex flex-col shadow-2xl">
            <div className="h-12 shrink-0 px-3 flex items-center justify-between border-b border-ink/10">
              <Logo />
              <button onClick={() => setSidebarOpen(false)} className={`text-ink/50 hover:text-ink p-2 -mr-1 rounded-[4px] hover:bg-flysch ${focusRing}`} aria-label="Close menu">
                <X className="h-4 w-4" />
              </button>
            </div>
            {renderSidebarBody(() => setSidebarOpen(false))}
          </aside>
        </div>
      )}

      {/* TWO-PANE */}
      <div className="flex-1 flex min-h-0">
        <aside className="hidden md:flex w-56 shrink-0 border-r border-ink/15 bg-paper flex-col min-h-0">
          {renderSidebarBody()}
        </aside>

        <main className="flex-1 flex flex-col min-w-0 bg-paper">
          <div className="flex-1 overflow-y-auto">
            <RenderCrashBoundary surface={`console.${active.id}`} resetKey={active.id}>
              {renderGroup(active)}
            </RenderCrashBoundary>
          </div>
        </main>
      </div>

      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} onNavigate={goTo} />
      <Toaster />
    </div>
  );
};
