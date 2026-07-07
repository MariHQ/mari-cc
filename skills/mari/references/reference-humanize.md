# humanize — apply the vendored Humanizer skill

Runs the external [blader/humanizer](https://github.com/blader/humanizer) skill — Wikipedia's
"Signs of AI writing" guide packaged as an agent skill — as the rewrite guide for the target.
Mari keeps a per-user checkout and treats that repo's `SKILL.md` as the authority for this
command. `deslop` remains Mari's native equivalent; use `humanize` when the user asks for it
by name or wants the Wikipedia-guide treatment specifically.

## Flow
1. Ensure the checkout: `mari humanize ensure`. First use clones the repo
   (needs network); after that it's cached and instant. The command prints the path to the
   skill's entry file (`SKILL.md`).
2. Read that `SKILL.md` and follow it as written on the target text — it defines its own
   process (draft → audit → final) and output format. Don't paraphrase it from memory; the
   checkout is the source of truth and it versions independently of Mari.
3. House context still applies on top: `PRODUCT.md` voice and register, and the `deslop`
   guardrails (rewrite, don't delete; never flatten a real voice).
4. Re-run the Mari detector afterwards — the humanize pass must not regress Family A, and
   detector findings the external skill doesn't cover still need fixing.

## Updating
"humanize update" → `mari humanize update`. This refreshes ONLY the vendored
humanizer checkout (fetch + hard reset to upstream); nothing else in Mari changes. Re-read
`SKILL.md` after an update. `mari humanize status` shows the current revision.

## Notes
- The checkout lives at `~/.mari/skills/humanizer` (per-user, shared across projects);
  override with `MARI_HUMANIZER_DIR`.
- If the clone fails (no network / no SSH key), say so and fall back to `deslop`.
