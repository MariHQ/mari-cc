import { useEffect, useState } from "react";
import { Cloud, Download, Upload, PlugZap } from "lucide-react";
import { api, type CloudStatus } from "@saas/lib/client";
import { Page, Field, Badge, btn, btnPrimary, focusRing, card } from "../console-ui";
import { toast } from "../feedback";

function relTime(iso: string | null): string {
  if (!iso) return "never";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return iso;
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

const inputCls =
  `w-full h-9 px-3 rounded-[4px] border border-ink/20 bg-paper text-[13px] text-ink placeholder:text-ink/40 outline-none focus:border-biscay-2 focus:ring-1 focus:ring-biscay-2/40 ${focusRing}`;
const labelCls = "font-term text-[10.5px] uppercase tracking-[0.08em] text-ink/55";

export function CloudGroup() {
  const [data, setData] = useState<CloudStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);

  // connect/init form
  const [backend, setBackend] = useState("s3");
  const [bucket, setBucket] = useState("");
  const [prefix, setPrefix] = useState("");
  const [region, setRegion] = useState("");

  // sync options
  const [compact, setCompact] = useState(false);
  const [noPush, setNoPush] = useState(false);

  function reload() {
    setLoading(true);
    setError(null);
    api
      .cloud()
      .then((d) => {
        setData(d);
        setBucket((d.cloud.bucket as string) || "");
        setPrefix((d.cloud.prefix as string) || "");
        setRegion((d.cloud.region as string) || "");
        setBackend((d.cloud.backend as string) || "s3");
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }

  useEffect(reload, []);

  async function run(key: string, fn: () => Promise<unknown>, ok: string) {
    setBusy(key);
    try {
      await fn();
      toast(ok, "success");
      reload();
    } catch (err) {
      toast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  const enabled = data?.enabled ?? false;
  const role = data?.role ?? "consumer";

  return (
    <Page
      title="Cloud"
      subtitle="Team sharing: push and pull the knowledge base to a shared warehouse."
      kicker="system"
      actions={
        enabled ? (
          <div className="flex items-center gap-2">
            <button
              onClick={() => run("pull", () => api.cloudPull(), "Pulled latest index")}
              disabled={busy !== null}
              className={`${btn} disabled:opacity-50`}
            >
              <Download size={15} /> {busy === "pull" ? "Pulling…" : "Pull"}
            </button>
            <button
              onClick={() => run("sync", () => api.cloudSync({ compact, noPush }), "Sync complete")}
              disabled={busy !== null || role !== "writer"}
              title={role !== "writer" ? "Only a writer can push" : undefined}
              className={`${btnPrimary} disabled:opacity-50`}
            >
              <Upload size={15} /> {busy === "sync" ? "Syncing…" : "Push / sync"}
            </button>
          </div>
        ) : undefined
      }
    >
      {loading && !data && !error && (
        <div className="mt-6 grid place-items-center py-16 text-[13px] text-ink/50">Loading…</div>
      )}

      {error && (
        <div className="mt-5 rounded-md border border-espelette/30 bg-espelette/[0.05] p-4">
          <div className="text-[13px] font-medium text-espelette">Couldn’t load cloud status</div>
          <div className="mt-1 font-term text-[12px] text-ink/70">{error}</div>
          <button onClick={reload} className="mt-3 font-term text-[12px] text-biscay-2 hover:underline">Retry</button>
        </div>
      )}

      {data && (
        <>
          {/* Status */}
          <div className={`${card} mt-5 p-4`}>
            <div className="flex items-center gap-2">
              <Cloud size={16} className="text-biscay-2" />
              <h4 className="text-[15px] font-semibold text-ink">Status</h4>
              <Badge label={enabled ? "enabled" : "disabled"} tone={enabled ? "ok" : "neutral"} />
              {enabled && <Badge label={role} tone={role === "writer" ? "info" : "neutral"} />}
            </div>
            <div className="mt-2">
              <Field label="Backend">
                <span className="font-term">{(data.storage.backend as string) || (data.cloud.backend as string) || "local"}</span>
              </Field>
              {data.storage.path ? (
                <Field label="Warehouse">
                  <span className="font-term break-all">{data.storage.path as string}</span>
                </Field>
              ) : (
                <>
                  <Field label="Bucket"><span className="font-term">{(data.cloud.bucket as string) || "—"}</span></Field>
                  <Field label="Prefix"><span className="font-term">{(data.cloud.prefix as string) || "—"}</span></Field>
                </>
              )}
              <Field label="Region"><span className="font-term">{(data.storage.region as string) || (data.cloud.region as string) || "—"}</span></Field>
              <Field label="Last pull"><span className="font-term">{relTime(data.lastPull)}</span></Field>
              <Field label="Retained snapshots"><span className="font-term">{String(data.storage.retain_snapshots ?? "—")}</span></Field>
            </div>
          </div>

          {enabled ? (
            <>
              {/* Role + sync options */}
              <div className={`${card} mt-5 p-4`}>
                <h4 className="text-[15px] font-semibold text-ink">This machine</h4>
                <p className="mt-1 text-[12.5px] text-ink/60">
                  A <span className="font-term">writer</span> can push local changes to the shared warehouse; a{" "}
                  <span className="font-term">consumer</span> only pulls.
                </p>
                <div className="mt-3 flex items-center gap-2">
                  {(["writer", "consumer"] as const).map((r) => (
                    <button
                      key={r}
                      onClick={() => run("role", () => api.cloudRole(r), `Role set to ${r}`)}
                      disabled={busy !== null || role === r}
                      className={`${role === r ? btnPrimary : btn} disabled:opacity-60`}
                    >
                      {r}
                    </button>
                  ))}
                </div>

                <div className="mt-4 flex flex-wrap items-center gap-4 border-t border-ink/10 pt-3">
                  <label className="flex items-center gap-2 text-[13px] text-ink/80">
                    <input type="checkbox" checked={compact} onChange={(e) => setCompact(e.target.checked)} />
                    Compact (expire snapshots, apply deletes) on next sync
                  </label>
                  <label className="flex items-center gap-2 text-[13px] text-ink/80">
                    <input type="checkbox" checked={noPush} onChange={(e) => setNoPush(e.target.checked)} />
                    Compact in place without pushing
                  </label>
                </div>
              </div>
            </>
          ) : (
            /* Connect / init */
            <div className={`${card} mt-5 p-4`}>
              <div className="flex items-center gap-2">
                <PlugZap size={16} className="text-biscay-2" />
                <h4 className="text-[15px] font-semibold text-ink">Connect a shared warehouse</h4>
              </div>
              <p className="mt-1 text-[12.5px] text-ink/60">
                Point this workspace at an S3 (or git) warehouse. <span className="font-term">Connect</span> joins an
                existing one; <span className="font-term">Initialize</span> creates it.
              </p>
              <div className="mt-3 grid gap-3 sm:grid-cols-2 max-w-[640px]">
                <label className="block">
                  <div className={labelCls}>Backend</div>
                  <select value={backend} onChange={(e) => setBackend(e.target.value)} className={`mt-1.5 ${inputCls}`}>
                    <option value="s3">s3</option>
                    <option value="git">git</option>
                  </select>
                </label>
                <label className="block">
                  <div className={labelCls}>Bucket</div>
                  <input value={bucket} onChange={(e) => setBucket(e.target.value)} placeholder="my-team-kb" className={`mt-1.5 font-term ${inputCls}`} />
                </label>
                <label className="block">
                  <div className={labelCls}>Prefix <span className="normal-case tracking-normal text-ink/40">(optional)</span></div>
                  <input value={prefix} onChange={(e) => setPrefix(e.target.value)} placeholder="mari/acme" className={`mt-1.5 font-term ${inputCls}`} />
                </label>
                <label className="block">
                  <div className={labelCls}>Region <span className="normal-case tracking-normal text-ink/40">(optional)</span></div>
                  <input value={region} onChange={(e) => setRegion(e.target.value)} placeholder="us-east-1" className={`mt-1.5 font-term ${inputCls}`} />
                </label>
              </div>
              <div className="mt-4 flex items-center gap-2">
                <button
                  onClick={() =>
                    run("connect", () => api.cloudConnect({ backend, bucket: bucket.trim(), prefix: prefix.trim() || undefined, region: region.trim() || undefined }), "Connected to warehouse")
                  }
                  disabled={busy !== null || !bucket.trim()}
                  className={`${btnPrimary} disabled:opacity-50`}
                >
                  {busy === "connect" ? "Connecting…" : "Connect"}
                </button>
                <button
                  onClick={() =>
                    run("init", () => api.cloudInit({ backend, bucket: bucket.trim(), prefix: prefix.trim() || undefined, region: region.trim() || undefined }), "Warehouse initialized")
                  }
                  disabled={busy !== null || !bucket.trim()}
                  className={`${btn} disabled:opacity-50`}
                >
                  {busy === "init" ? "Initializing…" : "Initialize"}
                </button>
              </div>
              <p className="mt-3 text-[12px] text-ink/45">
                S3 uses your ambient AWS credentials (the standard credential chain).
              </p>
            </div>
          )}
        </>
      )}
    </Page>
  );
}
