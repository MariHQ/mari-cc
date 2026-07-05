# audit — the human-facing detector report

Run the detector and report every finding, grouped by family, with a bad→good fix for each.
This is the front end of `mari detect`. Don't edit — produce the report.

## Flow
1. Run `mari detect <target>`.
2. Group findings by family: AI-slop tells · Clarity & concision · Style-guide conformance ·
   Inclusive & accessible.
3. For each finding give: the location, the offending span, and a concrete rewrite.
4. Lead with the `error`s, then `warn`, then `advisory`. Note the total per severity.
5. End with the 1–2 commands that would clear the most findings (usually `deslop` / `tighten`).

## Notes
- The detector never claims a document "is AI-written." Present findings as leads.
- `advisory` items are context-dependent — flag them, don't insist.
- Leans on **all rules**.
