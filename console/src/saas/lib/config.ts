// Runtime config for the local console. The API is same-origin (served by
// `mari console`), so there is almost nothing to configure — /config.json is
// fetched once on boot mostly to confirm the backend is reachable.

export type RuntimeConfig = {
  apiBase: string;
  local: boolean;
};

const FALLBACK: RuntimeConfig = { apiBase: "", local: true };

let cached: RuntimeConfig | null = null;

export const loadRuntimeConfig = async (): Promise<RuntimeConfig> => {
  if (cached) return cached;
  try {
    const r = await fetch("/config.json", { cache: "no-cache" });
    cached = r.ok ? { ...FALLBACK, ...(await r.json()) } : { ...FALLBACK };
  } catch {
    cached = { ...FALLBACK };
  }
  return cached;
};

export const getConfig = (): RuntimeConfig => cached ?? (cached = { ...FALLBACK });
