# Translations are suggested in self-hosted Weblate; git stays canonical

Contributors who don't use git suggest translations in a Weblate instance we host ourselves. Other
contributors vote on Suggestions, and a Reviewer for the locale accepts or declines them. Accepted
text reaches `locales/` only as a pull request, which a converter opens and a maintainer approves
and merges once CI is green. The TSV catalogs on `main` stay the only source of truth (ADR 0001). Weblate works on PO
files that the converter regenerates from `main` onto a `weblate` mirror branch, and nothing it
writes lands on `main` except through that PR.

Votes rank Suggestions; they never accept one. Weblate's auto-accept is off, so a run of friendly
upvotes can't ship a plausible but wrong term.

Shared strings take Corrections only (ADR 0003, 2026-10-31 amendment). A Reviewer declines a
preference on a `common.tsv` row and points the contributor to an app override. Overrides
themselves are not offered in Weblate for now. On a platform PR, the Reviewer's acceptance in
Weblate is the confirmation ADR 0003 asks for, and the PR lists every corrected shared row.

Contributors log in with GitHub or Discord, and before their first Suggestion they accept the
contribution licence (MIT OR Apache-2.0) and the clean-room rule.

The converter records the export each import is measured against. It never lets Weblate
overwrite a string whose translation changed on `main` since that export: the import refuses that
row and lists it for a maintainer.

Every external machine translation service and every shared translation memory is switched off,
because they would break the clean-room rule. Weblate keeps only this project's own memory.

## Consequences

- One more public service to run. Dylan owns it and the wider team is backup. A runbook
  (`docs/weblate-runbook.md`, written before launch) covers patching, backups, restore tests, TLS
  and spam.
- Only catalogs whose source of truth is here are offered: `common` and apps after cutover. An app
  joins Weblate at its cutover, so Weblate never competes with `tools/sync_app.py`.
- Locales without a Reviewer still collect Suggestions and Votes. Maintainers accept only obvious
  Corrections there: English left untranslated, the wrong script, a clear misspelling.
- The converter is now part of the contribution path. A bug in it can corrupt catalogs, so its
  round trip (TSV → PO → TSV with no change) is tested and runs in CI.
- Contributor accounts and emails live on our hardware, so a privacy notice is published before
  launch. `ATTRIBUTION.md` uses display names only.

## Considered options

- **Move the catalogs to PO**: rejected. It would rewrite every catalog, reverse ADR 0001, replace
  the TSV parser in `craft-libs/crates/i18n/src/catalog.rs` and move every app's runtime pin, and
  apps that haven't cut over would have to convert too. The converter gets Weblate PO's plurals and
  context without any of that.
- **Hosted Weblate (free for open-source projects)**: rejected. We are provisioning our own hardware
  and have an ops owner. Kept as the fallback if self-hosting stops being looked after.
- **Pontoon**: rejected. It is built around Mozilla's workflow, and we couldn't confirm it supports
  voting on Suggestions.
- **Auto-accept at N votes**: rejected, see above.
- **Auto-merge the sync PR on green CI**: not for now. CI checks the catalog format, not that the
  text was accepted in Weblate, and `main` doesn't require reviews yet. To revisit together with
  AutoCraft's merge queue.
- **Weblate commits straight to `main`**: rejected. It would skip the CI checker and the review of
  shared rows.
- **GitHub issue forms or a shared spreadsheet**: rejected. Neither has real voting or a Reviewer
  role, and a spreadsheet drifts away from the catalogs.
