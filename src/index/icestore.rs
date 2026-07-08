//! Object store for the Iceberg warehouse (SPEC §8.8/§9): one trait over a local
//! directory and an s3 bucket, so the manual writer emits the same bytes to
//! either. DuckDB stays read-only; this is the *write* side. Reads still go
//! through DuckDB's `cache_httpfs` (`iceberg_scan`).
//!
//! Everything addresses **full URIs** — a local filesystem path or an
//! `s3://bucket/key` — so the paths stamped into Iceberg metadata/manifests are
//! exactly what `iceberg_scan` later resolves.

use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use object_store::{ObjectStore, ObjectStoreExt};
use std::path::Path;
use std::sync::OnceLock;

/// One process-wide multi-thread runtime for all s3 I/O — building a runtime per
/// `Store::open` (called once per table-existence check) dominated s3 latency.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("building the shared tokio runtime for s3 I/O")
    })
}

pub enum Store {
    Local,
    S3 {
        bucket: String,
        client: object_store::aws::AmazonS3,
    },
}

impl Store {
    /// Build the store backing a warehouse URI. `s3://bucket/prefix` → S3 (region
    /// from `storage.region`, credentials from the environment chain); anything
    /// else is a local directory.
    pub fn open(warehouse: &str, region: &str) -> Result<Store> {
        if let Some(rest) = warehouse.strip_prefix("s3://") {
            let bucket = rest.split('/').next().unwrap_or("").to_string();
            if bucket.is_empty() {
                return Err(anyhow!("invalid s3 warehouse uri: {warehouse}"));
            }
            let mut builder = object_store::aws::AmazonS3Builder::from_env()
                .with_bucket_name(&bucket);
            if !region.is_empty() {
                builder = builder.with_region(region);
            }
            let client = builder
                .build()
                .with_context(|| format!("building s3 client for bucket {bucket}"))?;
            Ok(Store::S3 { bucket, client })
        } else {
            Ok(Store::Local)
        }
    }

    /// Of `tables`, those already published under `warehouse` (their
    /// `version-hint.text` exists). One `list` for s3 (not a head per table);
    /// direct stats for local.
    pub fn published_tables(&self, warehouse: &str, tables: &[&str]) -> Vec<String> {
        match self {
            Store::Local => tables
                .iter()
                .filter(|t| {
                    Path::new(warehouse)
                        .join(t)
                        .join("metadata")
                        .join("version-hint.text")
                        .exists()
                })
                .map(|t| t.to_string())
                .collect(),
            Store::S3 { bucket, client } => {
                let prefix = warehouse.strip_prefix(&format!("s3://{bucket}/")).unwrap_or("");
                let root = object_store::path::Path::from(prefix);
                let keys: Vec<String> = runtime().block_on(async {
                    let mut out = Vec::new();
                    let mut stream = client.list(Some(&root));
                    while let Some(Ok(meta)) = stream.next().await {
                        out.push(meta.location.to_string());
                    }
                    out
                });
                tables
                    .iter()
                    .filter(|t| {
                        let needle = format!("/{t}/metadata/version-hint.text");
                        keys.iter().any(|k| k.ends_with(&needle))
                    })
                    .map(|t| t.to_string())
                    .collect()
            }
        }
    }

    /// Mirror the remote warehouse to `local_dir`, preserving structure, so reads
    /// run against local files instead of paying per-object s3 round-trips on
    /// every query. Incremental: data/manifest/metadata files are immutable
    /// (append-only), so anything already present is skipped; only the tiny
    /// `version-hint.text` pointers are always refreshed to pick up new snapshots.
    /// A no-op for a local store.
    pub fn mirror(&self, warehouse: &str, local_dir: &str) -> Result<()> {
        let Store::S3 { bucket, client } = self else {
            return Ok(());
        };
        let prefix = warehouse.strip_prefix(&format!("s3://{bucket}/")).unwrap_or("");
        let root = object_store::path::Path::from(prefix);
        runtime().block_on(async {
            let mut stream = client.list(Some(&root));
            let mut gets = Vec::new();
            while let Some(meta) = stream.next().await {
                let meta = meta?;
                let key = meta.location.to_string();
                let rel = key
                    .strip_prefix(&format!("{prefix}/"))
                    .unwrap_or(&key)
                    .to_string();
                let local = format!("{local_dir}/{rel}");
                let always = rel.ends_with("version-hint.text");
                if !always && Path::new(&local).exists() {
                    continue;
                }
                gets.push((meta.location, local));
            }
            // Fetch in parallel — the metadata chain is otherwise latency-bound.
            let fetches = gets.into_iter().map(|(loc, local)| {
                async move {
                    let bytes = client.get(&loc).await?.bytes().await?;
                    if let Some(p) = Path::new(&local).parent() {
                        std::fs::create_dir_all(p).ok();
                    }
                    std::fs::write(&local, &bytes)
                        .with_context(|| format!("writing mirror file {local}"))?;
                    Ok::<(), anyhow::Error>(())
                }
            });
            futures::future::try_join_all(fetches).await?;
            Ok::<(), anyhow::Error>(())
        })
        .with_context(|| format!("mirroring {warehouse} to {local_dir}"))
    }

    /// Full URIs of every object/file under `prefix_uri` (a table dir). Used by
    /// compaction to find orphan files to delete.
    pub fn list_uris(&self, prefix_uri: &str) -> Result<Vec<String>> {
        match self {
            Store::Local => {
                let mut out = Vec::new();
                fn walk(dir: &Path, out: &mut Vec<String>) {
                    let Ok(rd) = std::fs::read_dir(dir) else {
                        return;
                    };
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            walk(&p, out);
                        } else {
                            out.push(p.to_string_lossy().to_string());
                        }
                    }
                }
                walk(Path::new(prefix_uri), &mut out);
                Ok(out)
            }
            Store::S3 { bucket, client } => {
                let prefix = prefix_uri.strip_prefix(&format!("s3://{bucket}/")).unwrap_or("");
                let root = object_store::path::Path::from(prefix);
                let uris = runtime().block_on(async {
                    let mut out = Vec::new();
                    let mut stream = client.list(Some(&root));
                    while let Some(meta) = stream.next().await {
                        out.push(format!("s3://{bucket}/{}", meta?.location));
                    }
                    Ok::<Vec<String>, anyhow::Error>(out)
                })?;
                Ok(uris)
            }
        }
    }

    /// Delete the object/file at `uri`.
    pub fn delete(&self, uri: &str) -> Result<()> {
        match self {
            Store::Local => {
                std::fs::remove_file(uri).with_context(|| format!("deleting {uri}"))
            }
            Store::S3 { bucket, client } => {
                let path = Self::s3_key(bucket, uri)?;
                runtime()
                    .block_on(client.delete(&path))
                    .with_context(|| format!("s3 delete {uri}"))?;
                Ok(())
            }
        }
    }

    /// The object key inside the bucket for an `s3://bucket/key` URI.
    fn s3_key(bucket: &str, uri: &str) -> Result<object_store::path::Path> {
        let prefix = format!("s3://{bucket}/");
        let key = uri
            .strip_prefix(&prefix)
            .ok_or_else(|| anyhow!("uri {uri} is not under bucket {bucket}"))?;
        Ok(object_store::path::Path::from(key))
    }

    /// Write `bytes` at `uri`. Local writes create parent dirs; s3 puts the
    /// object (atomic per key).
    pub fn put(&self, uri: &str, bytes: Vec<u8>) -> Result<()> {
        match self {
            Store::Local => {
                let p = Path::new(uri);
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                std::fs::write(p, &bytes).with_context(|| format!("writing {uri}"))
            }
            Store::S3 { bucket, client } => {
                let path = Self::s3_key(bucket, uri)?;
                let payload: object_store::PutPayload = bytes.into();
                runtime().block_on(client.put(&path, payload))
                    .with_context(|| format!("s3 put {uri}"))?;
                Ok(())
            }
        }
    }

    /// Write `bytes` at `uri` **only if it does not already exist**. Returns
    /// `true` if written, `false` if another writer already created it — the
    /// atomic compare-and-swap that lets concurrent writers commit safely (§9):
    /// each snapshot's `vN.metadata.json` can be created by exactly one writer.
    pub fn put_if_absent(&self, uri: &str, bytes: Vec<u8>) -> Result<bool> {
        match self {
            Store::Local => {
                let p = Path::new(uri);
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::OpenOptions::new().write(true).create_new(true).open(p) {
                    Ok(mut f) => {
                        use std::io::Write;
                        f.write_all(&bytes).with_context(|| format!("writing {uri}"))?;
                        Ok(true)
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                    Err(e) => Err(e).with_context(|| format!("creating {uri}")),
                }
            }
            Store::S3 { bucket, client } => {
                use object_store::{PutMode, PutOptions};
                let path = Self::s3_key(bucket, uri)?;
                let payload: object_store::PutPayload = bytes.into();
                let opts = PutOptions { mode: PutMode::Create, ..Default::default() };
                let res = runtime().block_on(client.put_opts(&path, payload, opts));
                match res {
                    Ok(_) => Ok(true),
                    Err(object_store::Error::AlreadyExists { .. }) => Ok(false),
                    Err(e) => Err(anyhow::Error::from(e)).with_context(|| format!("s3 create {uri}")),
                }
            }
        }
    }

    /// Read the bytes at `uri`, or `None` if it does not exist.
    pub fn get(&self, uri: &str) -> Result<Option<Vec<u8>>> {
        match self {
            Store::Local => match std::fs::read(uri) {
                Ok(b) => Ok(Some(b)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e).with_context(|| format!("reading {uri}")),
            },
            Store::S3 { bucket, client } => {
                let path = Self::s3_key(bucket, uri)?;
                let res: Result<Option<Vec<u8>>> = runtime().block_on(async {
                    match client.get(&path).await {
                        Ok(r) => Ok(Some(r.bytes().await?.to_vec())),
                        Err(object_store::Error::NotFound { .. }) => Ok(None),
                        Err(e) => Err(anyhow::Error::from(e)),
                    }
                });
                res.with_context(|| format!("s3 get {uri}"))
            }
        }
    }

}
