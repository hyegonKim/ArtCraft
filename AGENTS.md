# Working on ArtCraft

These rules apply to every coding agent (Claude, Codex, Antigravity/Gemini) and to people.

## One working copy

- The only working copy is `C:\ct\artcraft`. Write code here and nowhere else.
- `E:\Projects\AI_BibeCoding\ArtCraft` was the working copy until 2026-11-26, when its disk failed:
  files became unreadable and git could no longer read its own objects or write its index. It is
  retired. Do not open, build in, or copy files from it.
- Do not edit, build in, or copy files from any other checkout either: `C:\Projects\AI_BibeCoding\ArtCraft`,
  `*_C_residual\ArtCraft`, `ArtCraft-hevc-validation`, `ArtCraft-portable-fixtures-pr`,
  `artcraft-github-integration-*`. They are retired.
- Never copy files from one checkout into another. Copying carries edits but not deletions, which is
  how 1,510 deleted files came back and broke the soundcraft and lightcraft builds (2026-11-26).
- Do not create extra clones or `git worktree`s. Use a branch in this checkout instead.

## Git

- GitHub (`hyegonKim/ArtCraft`) is the source of truth; `main` is the integration branch.
- Start work with `git pull` on `main`, then create a branch such as `claude/<topic>`,
  `codex/<topic>` or `agy/<topic>` so it is clear which agent owns it.
- Commit small and push the branch the same day, so no work exists on only one disk.
- Merge through a pull request into `main`, then delete the branch.
- Before staging, check for zero-filled or unreadable files. The failed E: disk produced both, and
  some may have reached `main` before it was retired.

## Builds

- Each app folder (`filmcraft/`, `photocraft/`, …) is its own Cargo workspace; run cargo inside it.
- C: has little free space (about 30 GB on 2026-11-26). If a build runs out of room, delete the
  `target/` folders of apps you are not working on, or build with `CARGO_INCREMENTAL=0`.
