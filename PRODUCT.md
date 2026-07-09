# Mari

Mari helps teams curate their product knowledge for AI agents. 

As Claude becomes a daily work interface for engineering, product, marketing, and support teams, as well as external customers, companies need a new layer of infrastructure: curated project memory.

Enterprise search tools like Glean help people find company knowledge, but they are not designed for teams to continuously curate the context that AI agents use in their workflows. Search answers the question, “Where is the information?” Mari answers the question, “What should our AI know, trust, and reuse?”

Mari is a local-first Claude plugin that lets teams curate, search, and share their own product knowledge layer. Teams can maintain context in Git, local files, S3, shared embeddings, or repo-native documentation workflows, making Claude more useful without adding another external LLM wrapper, paying for a saas service, or duplicating AI spend.

Glean is for company-wide knowledge. Mari is for project-level knowledge.

## **Product Capabilities**

**Ingest and search team knowledge**  
Mari connects to the knowledge teams already use: GitHub issues, git commit messages, Linear tickets, Jira issues, Slack threads, Granola meeting notes, docs, local repositories, and product files. It retrieves, indexes, and makes that context available to Claude. Mari can leverage gdoc cli commands and personal access tokens for a frictionless experience.

Teams can choose how knowledge is stored and shared. Embeddings can stay local, sync through Git LFS or S3, or be managed through Mari’s SaaS layer. Cron jobs or hosted sync keep the index fresh, so Claude can answer with current team context.

Example:  
`/mari init search`  
`/search why did we change pricing tiers`  
`/search theres an outage in #incidents, what is causing it`  
`/search recent slack messages`  
`/sync`

**Curate what Claude should trust**  
Mari gives teams a way to actively maintain the context their AI agents use. Teams can tag knowledge as canonical, stale, deprecated, draft, internal, customer-facing, or needs review.

This turns product knowledge from passive search results into a managed memory layer. Instead of asking “what did the search engine find?”, teams can define “what should Claude trust?”

Example:  
`/tag docs/pricing.md canonical`  
`/tag old-onboarding-flow.md stale`  
`/mari glossary` to pull approved terms and phrases into the style system

**Improve AI-authored content**  
Mari gives teams a shared editorial vocabulary for working with Claude. Commands like `/deslop`, `/tighten`, `/sharpen`, `/understate`, `/clarify`, `/critique`, and `/polish` help users turn rough drafts and AI-generated copy into durable product knowledge.

These commands can run through `/mari`, or teams can pin frequent actions as standalone Claude commands.

Example:  
`/deslop README.md` strips out AI tells, clichés, and generic phrasing.  
`/tighten the changelog` cuts wordiness and filler.  
`/clarify the error copy` rewrites confusing UX text.  
`/critique docs/intro.md` reviews argument, structure, clarity, and voice.  
`/polish launch-announcement.md` prepares copy for publishing.

**Catch contradictions and unsupported claims**  
Mari checks claims against team knowledge, source-of-truth files, and structured facts. It can flag contradictions, missing evidence, and unsupported statements before they make it into docs, launch copy, support articles, or customer-facing pages.

Example:  
`/mari extract facts from recent slack messages in #product`

`/factcheck pricing-page.md` checks claims against `FACTS.md` and product knowledge.  
`/factcheck launch-post.md --source PRODUCT.md` flags claims that conflict with the current product definition.

**Enforce quality with deterministic hooks**  
Mari can run deterministic checks whenever Claude edits or creates content. These hooks catch common AI failure modes: vague phrasing, overclaiming, banned words, broken structure, unsupported claims, stale terminology, weak link text, and style guide violations.

This gives teams agent guardrails without relying entirely on another LLM to judge the first LLM.

Example:  
Claude edits `docs/api/auth.md`; Mari detects inflated claims, missing specifics, and terminology that violates the style guide.  
Claude edits API code; Mari reminds it to update the related API docs.  
A marketing page uses deprecated positioning; Mari flags it before publish.

**Generate and maintain documentation**  
Mari helps teams create and maintain documentation systems, not just individual pages. It can guide setup for doc platforms, derive structure from the codebase, generate API docs, write getting started guides, and validate the result.

Example:  
`/mari docsite` can document an entire codebase: choose a platform, derive architecture, write pages, add community files, and validate the documentation system.  
`/mari draft` can outline and write a new guide in the team’s voice.  
`/mari outline` can plan the structure of a new RFC, launch doc, or support article before Claude writes it.

**Support every document workflow**  
Mari gives Claude document-specific skills for the formats teams already use: runbooks, ADRs, postmortems, RFCs, PRDs, launch plans, support articles, pull request templates, and customer docs.

Teams can bring their own templates. When Claude drafts a document, Mari can point it to the right structure and required sections.

Example:  
Claude starts a pull request; Mari points it to the team’s PR template.  
Claude writes an incident review; Mari uses the company’s postmortem format.  
Claude drafts an RFC; Mari checks that tradeoffs, alternatives, rollout plan, and open questions are included.

**Localize and keep translations in sync**  
Mari understands common documentation formats and can compare localized versions of the same content. It can detect missing or outdated passages across languages and remind Claude to update translations when source content changes.

Example:  
Claude edits `docs/en/pricing.md`; Mari checks whether corresponding sections in `docs/es/pricing.md` and `docs/fr/pricing.md` need updates.  
`/mari localize` prepares copy for translation and global English.

**Map product knowledge over time**  
Mari builds a context graph from links, embedding neighbors, file history, source references, and document relationships. This lets teams explore product knowledge as a living system, not a folder of disconnected files.

Users can trace document lineage, understand what depends on what, find related decisions, and see where stale knowledge may affect current work.

Hooks on git commits can automatically associate commits with relevant conversations, issues, and find missing docs, so context is never lost.

Example:  
A pricing page links back to the launch plan, support objections, sales enablement notes, and implementation PRs.  
A deprecated API guide points to the replacement endpoint, migration guide, changelog, and customer support macros.

**Audit the knowledge base**  
Mari can audit existing docs and product knowledge to find stale pages, contradictions, missing links, duplicated content, unsupported claims, inconsistent terminology, and content that no longer matches the product.

Example:  
`/audit docs/` finds stale docs, weak claims, broken structure, and missing updates.  
`/mari audit` runs mechanical checks like readability, grammar, inclusivity, and link text.  
`/critique launch-plan.md` gives a higher-level editorial review of structure, clarity, and argument.
