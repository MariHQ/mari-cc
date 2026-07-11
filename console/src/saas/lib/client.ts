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

export type Status = {
  workspace: string;
  catalog: string;
  lastSync: string | null;
  embeddingModel: string;
  staleDays: number;
  counts: { documents: number; chunks: number; tags: number; lineageEdges: number };
  cloudEnabled: boolean;
};

export type TagCount = { status: string; n: number };
export type SyncEvent = {
  source_id: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  docs_seen: number;
  docs_changed: number;
  error: string | null;
};
export type Overview = {
  kpis: { documents: number; sourcesConnected: number; proposedLineage: number; tags: number };
  tagCounts: TagCount[];
  freshness: { fresh: number; stale: number };
  perSource: { provider: string; documents: number }[];
  recentSyncs: SyncEvent[];
};

export type TrackedRefs = { key: string; refs: string[] };
export type Source = {
  source: string;
  authProvider: string | null;
  credentialFree: boolean;
  connected: boolean;
  scope: string;
  tracked: TrackedRefs[];
  indexedDocuments: number;
  lastSyncAt: string | null;
  lastError: string | null;
  config: Record<string, unknown>;
};

export type DocumentRow = {
  doc_id: string;
  title: string | null;
  path: string | null;
  canonical_ref: string;
  url: string | null;
  provider: string;
  kind: string;
  updated_at: string | null;
  author_name: string | null;
  tag: string | null;
};

export type DocumentDetail = {
  document: DocumentRow & {
    mime_type: string | null;
    created_at: string | null;
    body: string;
    metadata_json: string;
    tagNote: string | null;
  };
  chunks: {
    chunk_id: string;
    chunk_index: number;
    heading_path: string;
    start_line: number;
    end_line: number;
    token_count: number;
  }[];
  lineage: {
    id: string;
    status: string;
    rel: string;
    confidence: number;
    by: string;
    fromRef: string;
    toRef: string;
  }[];
};

export type SearchHit = {
  doc_id: string;
  chunk_id: string;
  source: string;
  canonical_ref: string;
  title: string | null;
  path: string | null;
  url: string | null;
  author: string | null;
  updated_at: string | null;
  heading_path: string;
  start_line: number;
  end_line: number;
  score: number;
  tag: string | null;
  tag_note: string | null;
  replacement: string | null;
  matched_terms: string[];
  text: string;
};

export type TagRow = {
  target_type: string;
  target_id: string;
  status: string;
  note: string;
  by: string;
  at: string;
  ref: string;
  title: string | null;
};

export type LineageEdge = {
  id: string;
  status: string;
  rel: string;
  confidence: number;
  by: string;
  metadata: string;
  fromPath: string;
  fromStart: number;
  fromEnd: number;
  toPath: string;
  toStart: number;
  toEnd: number;
};

export type Project = {
  id: string;
  slug: string;
  documents: number;
  lastSync: string | null;
  path: string | null;
  active: boolean;
};
export type ProjectsResponse = { projects: Project[]; activeId: string; activePath: string };

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

export type CoverageFinding = { score: number; line: number; text: string };
export type CoverageResult = { flagged: CoverageFinding[]; ok?: boolean; error?: string };
export type RepoFile = { path: string; content: string; truncated: boolean };

export type DocsitePhase = { phase: string; command: string; output: string };
export type DocsiteStatus = {
  root: string;
  platform: string | null;
  docs_dir: boolean;
  readme: boolean;
  license: boolean;
  contributing: boolean;
  code_of_conduct: boolean;
  security: boolean;
  changelog: boolean;
  hook_configured: boolean;
  rules_configured: boolean;
  next_commands: string[];
};
export type DocsiteInfo = { plan: { phases: DocsitePhase[] }; status: DocsiteStatus };

export type CloudStatus = {
  enabled: boolean;
  role: string;
  lastPull: string | null;
  cloud: { enabled?: boolean; backend?: string; bucket?: string; prefix?: string; region?: string };
  storage: { backend?: string; path?: string; region?: string; retain_snapshots?: number };
};

export type ConfigPath = { path: string; type: string };
export type ConfigResponse = {
  effective: Record<string, unknown>;
  paths: ConfigPath[];
  global: Record<string, unknown>;
  repo: Record<string, unknown>;
};

/* ── API ─────────────────────────────────────────────────────────────────── */

export const api = {
  status: () => get<Status>("/api/status"),
  overview: () => get<Overview>("/api/overview"),

  sources: () => get<{ sources: Source[] }>("/api/sources"),
  track: (source: string, reference: string, action: "add" | "remove", listKey?: string) =>
    post<{ ok: boolean }>("/api/sources/track", { source, ref: reference, action, listKey }),
  sync: (source?: string) => post<{ ok: boolean; exitCode: number }>("/api/sources/sync", { source }),

  documents: (opts: { q?: string; source?: string; tag?: string; limit?: number } = {}) =>
    get<{ documents: DocumentRow[] }>(`/api/documents${qs(opts)}`),
  document: (id: string) => get<DocumentDetail>(`/api/documents/${encodeURIComponent(id)}`),

  search: (opts: { q: string; k?: number; source?: string; tag?: string }) =>
    get<{ query: string; hits: SearchHit[] }>(`/api/search${qs(opts)}`),

  tags: () => get<{ tags: TagRow[]; statuses: string[] }>("/api/tags"),
  applyTag: (reference: string, status: string, note?: string, supersededBy?: string) =>
    post<{ ok: boolean }>("/api/tags", { ref: reference, status, note, supersededBy }),
  removeTag: (reference: string) =>
    del<{ ok: boolean }>(`/api/tags${qs({ ref: reference })}`),

  lineage: () => get<{ edges: LineageEdge[] }>("/api/lineage"),
  addLineage: (from: string, to: string, by?: string, note?: string) =>
    post<{ ok: boolean }>("/api/lineage", { from, to, by, note }),
  confirmLineage: (id: string) => post<{ ok: boolean }>(`/api/lineage/${id}/confirm`),
  rejectLineage: (id: string) => post<{ ok: boolean }>(`/api/lineage/${id}/reject`),

  facts: () => get<{ file: string; items: { claim: string }[]; raw: string }>("/api/facts"),
  glossary: () => get<{ file: string; terms: { use: string; variants: string[] }[] }>("/api/glossary"),

  config: () => get<ConfigResponse>("/api/config"),
  setConfig: (path: string, value: unknown, scope: "repo" | "global" = "repo") =>
    put<{ ok: boolean; rebuildReminder: boolean }>("/api/config", { path, value, scope }),

  projects: () => get<ProjectsResponse>("/api/projects"),
  switchProject: (target: { path?: string; workspaceId?: string }) =>
    post<{ ok: boolean; activeId: string; path: string }>("/api/projects/switch", target),
  registerProject: (path: string) => post<{ ok: boolean; id: string }>("/api/projects/register", { path }),

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

  templates: () => get<{ templates: Template[] }>("/api/templates"),
  scaffoldTemplate: (type: string, title?: string, force?: boolean) =>
    post<{ ok: boolean }>("/api/templates/scaffold", { type, title, force }),

  setTagStatuses: (statuses: string[]) => post<{ ok: boolean }>("/api/tags/statuses", { statuses }),

  localization: () => get<Localization>("/api/localization"),
  localizationCoverage: (source: string, translation: string) =>
    get<CoverageResult>(`/api/localization/coverage${qs({ source, translation })}`),
  repoFile: (path: string) => get<RepoFile>(`/api/localization/file${qs({ path })}`),
  docsite: () => get<DocsiteInfo>("/api/docsite"),

  cloud: () => get<CloudStatus>("/api/cloud"),
  cloudPull: () => post<{ ok: boolean; exitCode: number }>("/api/cloud/pull"),
  cloudSync: (opts: { compact?: boolean; noPush?: boolean; retain?: number } = {}) =>
    post<{ ok: boolean; exitCode: number }>("/api/cloud/sync", opts),
  cloudRole: (role: "writer" | "consumer") => post<{ ok: boolean }>("/api/cloud/role", { role }),
  cloudConnect: (o: { backend: string; bucket: string; prefix?: string; region?: string }) =>
    post<{ ok: boolean }>("/api/cloud/connect", o),
  cloudInit: (o: { backend: string; bucket: string; prefix?: string; region?: string; force?: boolean }) =>
    post<{ ok: boolean }>("/api/cloud/init", o),
};
