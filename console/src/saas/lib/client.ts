// Typed client for the local mari console API (served by `mari console`).
// Same-origin JSON over fetch — no auth, no cloud. Every function maps to one
// handler in src/console/api.rs.

async function req<T>(path: string, init?: RequestInit): Promise<T> {
  const r = await fetch(path, {
    headers: { "content-type": "application/json" },
    ...init,
  });
  const text = await r.text();
  let data: unknown = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    /* non-JSON error body */
  }
  if (!r.ok) {
    const msg =
      (data as { error?: string } | null)?.error || `${r.status} ${r.statusText}`;
    throw new Error(msg);
  }
  return data as T;
}

const get = <T>(path: string) => req<T>(path);
const post = <T>(path: string, body?: unknown) =>
  req<T>(path, { method: "POST", body: body ? JSON.stringify(body) : undefined });
const put = <T>(path: string, body?: unknown) =>
  req<T>(path, { method: "PUT", body: body ? JSON.stringify(body) : undefined });
const del = <T>(path: string) => req<T>(path, { method: "DELETE" });

const qs = (params: Record<string, string | number | undefined | null>) => {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== "") p.set(k, String(v));
  }
  const s = p.toString();
  return s ? `?${s}` : "";
};

/* ── types ───────────────────────────────────────────────────────────────── */

export type Nudge = {
  name: string;
  when: unknown;
  edit: unknown[];
  message: string;
  exclude: string[];
};
export type EditRule = { name: string; paths: string[]; notify: string; exclude: string[] };

export type DetectorRule = { id: string; family: string; pack: string | null };
export type DetectorInfo = {
  styleGuide: string;
  zeroTolerance: string[];
  ignoreRules: string[];
  ignoreFiles: string[];
  grammar: boolean;
  catalog: DetectorRule[];
};

export type DetectorListKind = "words" | "phrases" | "weighted" | "map" | "groups";
export type DetectorList = {
  id: string;
  label: string;
  family: string;
  pack: string | null;
  kind: DetectorListKind;
  default: unknown[];
  override: unknown[] | null;
  overridden: boolean;
  source: "repo" | "global" | "default";
};

export type Template = { id: string; title: string; file: string; sections: string[]; basis: string };

export type DetectFinding = {
  ruleId: string;
  family: string;
  severity: string;
  message: string;
  span: string;
  offset: number;
  length: number;
  line: number;
  col: number;
};
export type DetectScore = {
  score: number;
  band: string;
  words: number;
  findingCount: number;
  per1k: number;
  byFamily: Record<string, number>;
  contractions: number;
  firstPerson: number;
  discount: number;
};
export type DetectResult = {
  path: string;
  styleGuide: string;
  wordCount: number;
  score: DetectScore;
  findings: DetectFinding[];
};

export type LocalizationCell = {
  path: string;
  layout: string;
  stale: boolean;
  issues: string[];
  ok: boolean;
};
export type LocalizationSource = { source: string; translations: Record<string, LocalizationCell> };
export type Localization = { languages: string[]; sources: LocalizationSource[]; sourceLangs: string[] };

export type RepoFile = { path: string; content: string; truncated: boolean };

export type ConfigPath = { path: string; type: string };
export type ConfigResponse = {
  effective: Record<string, unknown>;
  paths: ConfigPath[];
  repo: Record<string, unknown>;
};

/* ── API ─────────────────────────────────────────────────────────────────── */

export const api = {
  glossary: () => get<{ file: string; terms: { use: string; variants: string[] }[] }>("/api/glossary"),

  config: () => get<ConfigResponse>("/api/config"),
  setConfig: (path: string, value: unknown, scope: "repo" = "repo") =>
    put<{ ok: boolean; rebuildReminder: boolean }>("/api/config", { path, value, scope }),


  nudges: () => get<{ nudges: Nudge[] }>("/api/nudges"),
  addNudge: (n: { name: string; when: string; edit: string[]; message?: string; exclude?: string }) =>
    post<{ ok: boolean }>("/api/nudges", n),
  removeNudge: (name: string) => del<{ ok: boolean }>(`/api/nudges${qs({ name })}`),

  rules: () => get<{ rules: EditRule[] }>("/api/rules"),
  addRule: (r: { name: string; paths: string; notify: string; exclude?: string }) =>
    post<{ ok: boolean }>("/api/rules", r),
  removeRule: (name: string) => del<{ ok: boolean }>(`/api/rules${qs({ name })}`),
  discoverRules: () => post<{ rules: EditRule[] }>("/api/rules/discover"),

  detector: () => get<DetectorInfo>("/api/detector"),
  detect: (input: { text?: string; path?: string; style?: string }) =>
    post<DetectResult>("/api/detect", input),
  setZero: (rule: string, action: "add" | "remove") =>
    post<{ ok: boolean }>("/api/detector/zero", { rule, action }),
  setIgnore: (rule: string, action: "add" | "remove", reason?: string) =>
    post<{ ok: boolean }>("/api/detector/ignore", { rule, action, reason }),
  detectorLists: () => get<{ lists: DetectorList[] }>("/api/detector/lists"),
  setDetectorList: (id: string, value: unknown[], scope: "repo") =>
    put<{ ok: boolean }>("/api/detector/lists", { id, value, scope }),
  resetDetectorList: (id: string, scope: "repo") =>
    put<{ ok: boolean; reset: boolean }>("/api/detector/lists", { id, reset: true, scope }),

  templates: () => get<{ templates: Template[] }>("/api/templates"),
  scaffoldTemplate: (type: string, title?: string, force?: boolean) =>
    post<{ ok: boolean }>("/api/templates/scaffold", { type, title, force }),

  localization: () => get<Localization>("/api/localization"),
  repoFile: (path: string) => get<RepoFile>(`/api/localization/file${qs({ path })}`),
};
