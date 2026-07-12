# Share knowledge with your team

Mari is local-first, but the knowledge layer is meant to be shared. Your curation already travels with the repo. The index itself can stay on one machine or sync through infrastructure your team already controls.

## What is already shared

Tags, tracked sources, detector settings, edit-notify rules, nudges, the glossary, and the facts ledger all live in committed files (`.mari/config.json`, `STYLE.md`, `FACTS.md`, `PRODUCT.md`). Commit them and every teammate gets the same curated context. Nothing extra is needed for this layer.

## Storage choices for the index

The embeddings and catalog are larger and change often. You have a few options:

- **Local only.** Each teammate runs `mari sync` and builds their own index. Simplest, no shared storage.
- **Git backend.** Share the catalog through the repo with data files on Git LFS. A sync prints a reminder to commit the `.mari` catalog.
- **S3 backend.** Store the shared catalog in a bucket your team owns.

The backend is set in config under `cloud.backend` (`s3` or `git`).

## Set up team sync

`mari cloud` manages the shared replica:

```sh
mari cloud init            # publish this repo's index as the shared source
mari cloud connect         # consume a teammate's shared index
mari cloud role            # show whether this workspace is a producer or consumer
```

Read commands auto-pull the shared replica first when cloud is enabled. If the pull fails, they warn and read the last local copy rather than erroring.

## Keep the index fresh

Mari runs no background daemon and no built-in cron. To keep a shared index current, wire `mari sync` into a job your team already runs, such as a nightly continuous-integration task or a cron entry. Consumers cannot run `--rebuild` against a shared index. Rebuild locally, then publish again with `mari cloud init`.
