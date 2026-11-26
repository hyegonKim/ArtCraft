# Glossary

**Crafting App**: one of the storytold applications that share these translations (WordCraft,
PdfCraft, PhotoCraft, ArtCraft, …).

**Locale**: a language (optionally with region) that strings are translated into.

**Source text**: the English text of a string, as written in the app. It is also the string's key:
English is never translated, only looked up.

**Context**: an optional label that tells apart two strings with the same source text but
different meanings (e.g. "Open" the verb vs. "Open" the state).

**Catalog**: one locale's translations of a set of strings.

**Shared string**: a string used by more than one Crafting App, translated once in the shared
catalog.

**App string**: a string used by one Crafting App only, translated in that app's own catalog.

**Override**: an app catalog's translation of a shared string; the app uses it instead of the
shared one, in that locale only.

**Translation**: a locale's text for a source text (and context).

**Correction**: a change to a shared string's translation that fixes an error, wrong in every app
that uses it: a wrong meaning, a misspelling, broken grammar, English left untranslated, or the
wrong script. A different but valid wording is a preference, not a correction, and goes in an
override.

**Plural forms**: the variants of one translation chosen by a count (e.g. "1 page" / "2 pages");
each locale has as many as its language's plural rules need.

**Pseudo locale**: a fake locale that marks every translated string, used to find text that
bypasses translation. Never shipped as a choice to users.

**Pin**: the craft-i18n version a Crafting App's release embeds. The app moves it forward on purpose,
never automatically. "Pin" alone always means this one.

**Runtime pin**: the craft-libs version a Crafting App looks its translations up with. It decides how
translated text is shown (numbers, plural choice, direction marks), so moving it can change what
users see even when the pin stays put.

**Cutover**: the change after which a Crafting App takes its translations only from its pin and
keeps no catalogs of its own. Before it, new translations land in the app; after it, here.

**Freeze**: a pause on merging an app's catalog changes while its cutover lands, so no translation
is left behind in the app.

**Offered locale**: a locale a Crafting App lets its users choose. A locale the app has catalogs
for but doesn't offer (one its interface isn't ready for) still serves as a fallback.

**Language data**: the per-locale data that checks and breaks text, as opposed to translating the
interface: spelling dictionaries, hyphenation patterns and the suite word list. Never just
"dictionary", which is ambiguous.

**Spelling dictionary**: a locale's list of correct words and the rules that inflect them, taken
unchanged from an open-licensed upstream project.

**Hyphenation patterns**: a locale's rules for where a word may break at the end of a line, taken
unchanged from an open-licensed upstream project.

**Suite word list**: words every Crafting App accepts as correctly spelled (product names, units,
trade terms), written by the suite itself.

**User word**: a word a user or a document adds as correct. It belongs to the app it was added in,
not to Language data.

**Language build**: a release of a Crafting App that embeds the Language data of chosen locales
besides English, so those locales never need a download.

**Language pack**: one locale's Language data as a single download (its spelling dictionary,
hyphenation patterns and their licences), installed and removed together.
_Avoid_: dictionary download, language download

**Licence class**: how an upstream file's licence lets the suite ship it: **Embeddable** (may be
built into a release), **Download-only** (only as a separate, unmodified file in a Language pack)
or **Excluded** (never shipped).

**Suggestion**: a proposed translation from a logged-in contributor. It changes no catalog until a
Reviewer accepts it.
_Avoid_: submission, proposal

**Vote**: a contributor's support for one Suggestion. Votes rank Suggestions; they never accept one.

**Reviewer**: a trusted speaker of one locale who accepts or declines that locale's Suggestions. A
Reviewer is per locale, never global.
