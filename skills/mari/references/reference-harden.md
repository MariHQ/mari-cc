# harden — edge-case copy: errors, empty states, microcopy, i18n

The copy that only shows up when something goes wrong. The microcopy register is mandatory here.

## Flow
1. Load `register-microcopy.md` (non-optional) and run the detector on the strings.
2. Work the edge cases:
   - error messages follow the formula: what happened / why / how to fix — in plain words
   - empty, loading, and confirmation states say something useful, not "No data."
   - zero/one/many and plural forms are all handled
   - i18n length budgets: leave room for ~30% expansion; keep variables out of sentence grammar
3. Keep terms consistent with the glossary (`terminology-consistency`) and link text specific
   (`vague-link-text`).

## Guardrails
- Never blame the user. State the fix, not the fault.
- A truncated or hard-coded English string is a bug — flag it.

Leans on: `vague-link-text`, `terminology-consistency`, microcopy-register checks.
