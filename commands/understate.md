---
description: Cut over-explanation — the #1 durable AI tell
argument-hint: "<target>"
allowed-tools: Bash(mari *), Read, Edit
---

Run the **understate** editorial verb from the mari skill on $ARGUMENTS (a path, a natural-language reference like "the changelog", or — if omitted — the file(s) just edited this session; ask if none).

Setup first: load PRODUCT.md / STYLE.md / FACTS.md if present, read a representative file for voice, resolve the register, and run `mari detect <target>` for ground truth. Then apply the verb's reference flow: `references/reference-understate.md` in the mari skill.  Preserve the author's meaning and voice — rewrite, not delete. Finish by re-running `mari detect` and fix any findings the edit introduced.
