# Mari Console

The console is a local, single-user dashboard served by the `mari` binary. It provides views for the detector overview, rule catalog, 49 word lists, glossary, templates, localization, nudges, edit-notify rules, and repository configuration.

## Development

```sh
npm install
npm run dev
npm run build
```

The production bundle in `dist/` is embedded in the Rust binary. The backend lives in `src/console/` and binds to `127.0.0.1`.
