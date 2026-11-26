<!-- craft-house-rules:start core source=dde09ea31e2e457f5b4653a6e76f7cdf55944bdf sha256=540e6cea75d12d0710c3dca89658328e81e508ecef102446ca5bb95b7c0b92bb -->
## House rules

These apply in every Craft repository. A later rule may be stricter, not weaker. The source is `standards/house-rules-core.md` in `storytold/craft-contrib`. Propose changes there, not in a copy.

1. **A person is accountable.** Whoever runs an agent answers for what it opens and for its review comments.
2. **Check, then claim.** Before writing code, read the issue comments, search pull requests for `#N` and the keywords, and check recent commits on `main`. Comment on the issue first. Release a claim you will not finish.
3. **Verify, don't guess.** Open a definition before using it. Confirm the cause in the code, even when someone names a location.
4. **Keep fixes narrow and tested.** Fix what the issue describes, where the bug is. Add a test that fails without the fix.
5. **Review the diff before opening a pull request.** Identifiers exist as used, the test fails without the fix, nothing unrelated changed, and the description is true.
6. **Look for a better pull request.** If another fixes the same thing, close yours when it is better or equal, and say why.
7. **Report only what ran.** Say what you tested and what you could not run. Without a local build or test, say so and rely on CI before marking ready.
8. **`main` changes through a pull request.** Do not push to it. Branch protection enforces this. There is no exception.
9. **Stop and ask only about** licensing outside the allowlist, publishing, releases, a new remote, secrets, spending money, destructive git, and anything else hard to undo. Otherwise keep working. `plan/` is a gitignored notebook, not rules. Without it, do not ask for it and do not stop.
10. **Keep secrets and personal details out** of the repository, logs and requests.
11. **CI stays shared.** Workflows call `storytold/craft-common-actions`. Do not change CI in a feature or fix pull request. If CI cannot cover a change, open an issue there.
12. **Guards say why.** A check that looks removable says what it prevents. Read that before changing it.
13. **Instructions must be readable by everyone.** No notes about one machine, and no links to files a reader cannot open. `craftrules` is retired. Do not link to it.
14. **Keep pull requests moving.** Concurrent pull requests are fine; unanswered actionable feedback is not. Respond in 7 days, a reminder at 14, closure eligible at 30, from when an actionable request waits on the author. Suggestions and unconfirmed bot findings do not start the clock. A fix, explanation or agreed plan resets it; unrelated activity does not. Waiting on a reviewer or maintainer pauses it and does not count against the author. A maintainer decides closure; it is never automatic. Reuse search results, do not poll, and stop at a rate limit.
15. **No agent attribution.** A pull request does not say an agent wrote it. A repository rule may not require that.
<!-- craft-house-rules:end -->

# AGENTS.md

Shared translations for the Craft Apps: TSV catalogs in `locales/` and the checker CI runs.

- **Data, not code.** The lookup code is the `craft-i18n` crate in craft-libs
  ([ADR 0002](docs/adr/0002-runtime-in-craft-libs-data-in-craft-i18n.md)). This repo holds only
  data (the catalogs and Language data) and the checker (`crates/craft-i18n-tests`).
- **Catalogs:** the format and every rule CI checks are in `docs/catalog-format.md`; the words in
  `GLOSSARY.md`. Shared rows in `common.tsv` only grow, except for a correction
  ([ADR 0003](docs/adr/0003-shared-catalog-grows-additively.md)); an app changes one's wording with
  an override in its own `<app>.tsv`.
- **Clean room:** every translation is written from the plain meaning of the English. Never consult
  or copy another product's localisation, string table or translated interface. Language data
  (spelling dictionaries, hyphenation patterns) is the exception: unchanged open-licensed upstream
  files ([ADR 0004](docs/adr/0004-third-party-language-data.md)).
- **Credit:** new translations and migrated catalogs are recorded in `ATTRIBUTION.md`, including
  AI assistance and whether a native speaker reviewed them. Recording AI assistance in
  `ATTRIBUTION.md` is data provenance (how a translation was produced and whether a native speaker
  reviewed it), not agent attribution of the pull request; core rule 15 does not apply to it.
- **Tests for catalog rows:** core rule 4's "add a test that fails without the fix" applies to the
  checker and tools. A catalog-only change is covered by CI's catalog checks; it needs no new test.
- **Never crash.** The checker's `lib.rs` denies `unwrap`, `expect`, `panic` and friends; the
  workspace forbids `unsafe`.
- **Before merging:** `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`,
  `python3 -m unittest discover -s tools` (catalog sync/rekey helpers; always write/read TSV as UTF-8).
