import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import "@/index.css";
import { loadRuntimeConfig } from "./lib/config";
import App from "./App";
import { ScrollToTop } from "@/components/ScrollToTop";
import { RenderCrashBoundary } from "./components/RenderCrashBoundary";

const Splash = () => (
  <div className="min-h-screen w-full grid place-items-center bg-paper text-ink">
    <div className="flex items-center gap-3 text-sm font-term text-ink/60">
      <span className="h-2 w-2 rounded-full bg-biscay-2 animate-pulse" />
      Loading…
    </div>
  </div>
);

const Bootstrap = () => {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadRuntimeConfig()
      .catch((e) => console.error("[mari] failed to load runtime config", e))
      .finally(() => {
        if (!cancelled) setReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!ready) return <Splash />;

  return (
    <BrowserRouter>
      <ScrollToTop />
      <RenderCrashBoundary surface="console.root" resetKey="root">
        <App />
      </RenderCrashBoundary>
    </BrowserRouter>
  );
};

createRoot(document.getElementById("root")!).render(<Bootstrap />);
