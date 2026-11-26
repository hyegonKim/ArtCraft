# Working on ArtCraft

These rules apply to every coding agent (Claude, Codex, Antigravity/Gemini) and to people.

## One working copy

- The only working copy is `E:\Projects\AI_BibeCoding\ArtCraft`. Write code here and nowhere else.
- Do not edit, build in, or copy files from any other checkout: `C:\Projects\AI_BibeCoding\ArtCraft`,
  `*_C_residual\ArtCraft`, `ArtCraft-hevc-validation`, `ArtCraft-portable-fixtures-pr`,
  `artcraft-github-integration-*`. They are retired.
- Never copy files from one checkout into another. Copying carries edits but not deletions, which is
  how 1,510 deleted files came back and broke the soundcraft and lightcraft builds (2026-11-26).
- Do not create extra clones or `git worktree`s. Use a branch in this checkout instead.

## Git

- GitHub (`hyegonKim/ArtCraft`) is the source of truth; `main` is the integration branch.
- Start work with `git pull` on `main`, then create a branch such as `claude/<topic>`,
  `codex/<topic>` or `agy/<topic>` so it is clear which agent owns it.
- Commit small and push the branch the same day. The E: disk has bad sectors, so anything that
  only exists on E: can be lost.
- Merge through a pull request into `main`, then delete the branch.
- Before staging, check for zero-filled or unreadable files; the disk has produced both.

## Builds

- Each app folder (`filmcraft/`, `photocraft/`, …) is its own Cargo workspace; run cargo inside it.
- If a build fails with `os error 1392` (file corrupted), delete that workspace's `target/` and rebuild,
  or set `CARGO_TARGET_DIR` to a folder on C:.
