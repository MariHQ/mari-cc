---
description: Detector report for files, or audit the knowledge base
argument-hint: "[path]"
allowed-tools: Bash(mari *), Read, Edit
---

If the user says to audit the knowledge base (or names no path but means the KB), run `mari audit kb [--strict]`; otherwise run `mari audit $ARGUMENTS` for the human-facing detector report grouped by family with bad→good example fixes. Report only — never edit files from this command. Summarize the worst findings and offer the matching editorial verb (/deslop, /tighten, /clarify) as the fix path.
