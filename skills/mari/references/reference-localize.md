# localize — prepare copy for translation and global English

Make the text easy to translate and safe for a global audience. This is preparation, not
translation.

## Flow
1. Run the detector; read the clarity findings.
2. Prepare the copy:
   - simplify idioms and culture-bound references that won't survive translation
   - expand contractions and ambiguous phrasing where it aids a translator
   - keep variables out of sentence grammar (don't inflect around `{count}`); give translators
     full sentences, not fragments to assemble
   - flag length-budget risks (German runs ~30% longer; CJK differs) for UI strings
   - keep terms consistent (`terminology-consistency`) so a glossary can map them
3. Note anything that genuinely can't be localized (puns, wordplay) for the user.

Leans on: `wordy-phrase`, `terminology-consistency`, idiom checks.
