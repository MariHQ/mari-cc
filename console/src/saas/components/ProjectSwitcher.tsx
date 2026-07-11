import { useEffect, useRef, useState } from "react";
import { Boxes, Check, ChevronDown, Loader2 } from "lucide-react";
import { api, type Project } from "@saas/lib/client";
import { focusRing } from "./console-ui";
import { toast } from "./feedback";

function relTime(iso: string | null): string {
  if (!iso) return "never synced";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return iso;
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

/**
 * Switch which indexed project (workspace) the console operates on. The whole
 * tool addresses a project by its folder on disk, so a project is switchable
 * only once its path is known — which happens automatically the first time you
 * run `mari console` inside it. Projects indexed before that are shown but
 * disabled (we can't locate them), with a one-line hint.
 */
export function ProjectSwitcher() {
  const [open, setOpen] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [activeId, setActiveId] = useState<string>("");
  const [activePath, setActivePath] = useState<string>("");
  const [switching, setSwitching] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement | null>(null);

  function reload() {
    api
      .projects()
      .then((d) => {
        setProjects(d.projects);
        setActiveId(d.activeId);
        setActivePath(d.activePath);
      })
      .catch(() => {});
  }

  useEffect(reload, []);

  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, [open]);

  const active = projects.find((p) => p.id === activeId);
  const activeLabel = active?.slug ?? activePath.split("/").pop() ?? "project";

  async function switchTo(p: Project) {
    if (p.id === activeId) return setOpen(false);
    if (!p.path || switching) return;
    setSwitching(p.id);
    try {
      await api.switchProject({ workspaceId: p.id });
      // Reload so every section refetches against the new active project.
      window.location.assign("/console");
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
      setSwitching(null);
    }
  }

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((o) => !o)}
        className={`inline-flex items-center gap-1.5 min-w-0 max-w-[38vw] rounded-[4px] border border-ink/20 bg-paper px-2 sm:px-2.5 py-1.5 font-term text-[12px] text-ink hover:border-ink/45 transition-colors ${focusRing}`}
        title="Switch project"
      >
        <Boxes className="h-3.5 w-3.5 text-ink/50 shrink-0" />
        <span className="truncate">{activeLabel}</span>
        <ChevronDown className="h-3.5 w-3.5 text-ink/50 shrink-0" />
      </button>

      {open && (
        <div className="absolute left-0 top-[calc(100%+6px)] z-[80] w-[340px] rounded-md border border-ink/15 bg-paper shadow-2xl">
          <div className="px-3 py-2 border-b border-ink/10 font-term text-[10.5px] uppercase tracking-[0.12em] text-ink/50">
            Indexed projects
          </div>
          <div className="max-h-[340px] overflow-y-auto p-1.5">
            {projects.length === 0 && (
              <div className="px-3 py-6 text-center text-[12.5px] text-ink/55">
                No indexed projects found under ~/.mari.
              </div>
            )}
            {projects.map((p) => {
              const isActive = p.id === activeId;
              const locatable = !!p.path;
              return (
                <button
                  key={p.id}
                  onClick={() => switchTo(p)}
                  disabled={!locatable && !isActive}
                  title={locatable ? p.path ?? undefined : "Run `mari console` in this project to enable switching"}
                  className={`w-full flex items-center gap-2.5 px-2.5 py-2 rounded-[4px] text-left transition-colors ${focusRing}
                    ${locatable || isActive ? "hover:bg-flysch/70 cursor-pointer" : "opacity-55 cursor-not-allowed"}`}
                >
                  <Boxes className={`h-4 w-4 shrink-0 ${isActive ? "text-biscay-2" : "text-ink/45"}`} />
                  <span className="min-w-0 flex-1">
                    <span className="text-[13px] font-medium text-ink truncate block">{p.slug}</span>
                    <span className="block font-term text-[11px] text-ink/55">
                      {p.documents.toLocaleString()} docs · {relTime(p.lastSync)}
                    </span>
                  </span>
                  {switching === p.id ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin text-ink/50 shrink-0" />
                  ) : isActive ? (
                    <Check className="h-3.5 w-3.5 text-biscay-2 shrink-0" />
                  ) : !locatable ? (
                    <span className="font-term text-[10px] text-ink/40 shrink-0 whitespace-nowrap">not opened here</span>
                  ) : null}
                </button>
              );
            })}
          </div>
          <div className="border-t border-ink/10 px-3 py-2 font-term text-[11px] text-ink/45 leading-relaxed">
            A project becomes switchable once you run <span className="text-ink/70">mari console</span> inside it.
          </div>
        </div>
      )}
    </div>
  );
}
