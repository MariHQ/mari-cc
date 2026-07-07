<!--
  mari template: security — Security policy
  Grounding: GitHub's SECURITY.md convention

  Required sections (in order):
    1. Supported Versions        (aliases: supported version, affected versions)
    2. Reporting a Vulnerability (aliases: reporting security issues,
                                  reporting a security issue, report a vulnerability,
                                  how to report)
  Recommended sections:
    - Disclosure Policy       (aliases: disclosure, coordinated disclosure,
                               responsible disclosure)
    - Security Update Policy  (aliases: security updates, response)

  Tone norms:
    - Give a private reporting channel — never "just open an issue".
    - State a response-time expectation you can actually meet.
    - List exactly which versions receive security fixes.

  Detection heuristic (mari asset detect):
    - Canonical filename (+4, qualifying): SECURITY.md (also .mdx/.rst/.txt; optional
      language suffix), case-insensitive, anywhere in the tree.
    - Directory match (+3, qualifying): .github/ or docs/security/
    - Heading markers (+2 each, capped at 3 hits): supported versions, supported version,
      reporting a vulnerability, reporting security issues, reporting a security issue,
      report a vulnerability, how to report, disclosure, disclosure policy,
      coordinated disclosure, responsible disclosure, security updates, response,
      scope, safe harbor, preferred languages. Strong (content-only qualifying)
      markers: supported versions, reporting a vulnerability, coordinated disclosure,
      disclosure policy.
    - Best-scoring type wins; total score must be >= 4.

  Structure checks (mari asset check):
    - asset-missing-section (warn): each required section absent from headings.
    - asset-missing-section (advisory): each recommended section absent.

  Community-file role: required core artifact; `mari community` scaffolds it at
  SECURITY.md via this same asset template.

  Note: when a title is passed to `mari asset scaffold security <title>`, the H1
  becomes "# Security Policy — <title>"; with no title it is "# Security Policy".
-->
# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest  | ✅        |
| older   | ❌        |

## Reporting a Vulnerability

**Do not open a public issue for security problems.** Report privately via <security contact>
(or GitHub's private "Report a vulnerability" advisory). Include steps to reproduce and impact.

We aim to acknowledge reports within <N business days> and to keep you updated as we
investigate and ship a fix.

## Disclosure Policy

We follow coordinated disclosure: we'll agree a disclosure timeline with you and credit you
in the advisory unless you prefer otherwise.
