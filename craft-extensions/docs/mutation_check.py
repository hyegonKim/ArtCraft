"""Mutation check: break one safety rule at a time and confirm a test fails.

Run from the repository root:
  python docs/mutation_check.py core|store [--dry]

--dry only proves each pattern occurs the expected number of times. A mutant counts as caught only when
a test FAILED at run time (a mutant that does not compile proves nothing). Source is restored from the
text read at the start, and the file is touched afterwards so cargo never keeps a stale mutated build.
Run it with nothing else editing the sources.
"""
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (label, file, exact old text, new text[, expected match count])
CORE = [
    ("id: uppercase accepted", "id.rs", "matches!(c, 'a'..='z' | '0'..='9' | '.' | '-' | '_')", "matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_')"),
    ("id: first-party prefix needs exact match", "id.rs", ".any(|p| self.0.starts_with(p))", ".any(|p| self.0 == *p)"),
    ("id: single segment allowed", "id.rs", "if segments < 2 {", "if segments < 1 {"),
    ("path: dot segments allowed", "path.rs", 'if segment == "." || segment == ".." {', "if false {"),
    ("path: reserved device names allowed", "path.rs", "if is_reserved_device(segment) {", "if false {"),
    ("path: folder spelling collision ignored", "path.rs", "Some(seen) if *seen != prefix =>", "Some(seen) if *seen != prefix && false =>"),
    ("path: depth unchecked", "path.rs", "if depth > MAX_DEPTH {", "if false {"),
    ("path: trailing dot allowed", "path.rs", "if segment.ends_with('.') {", "if false {"),
    ("text: invisible formatting allowed", "text.rs", "} else if is_invisible_formatting(c) {", "} else if false && is_invisible_formatting(c) {"),
    ("text: control characters allowed", "text.rs", "} else if c.is_control() {", "} else if false && c.is_control() {"),
    ("version: leading zero allowed", "version.rs", "(part.len() > 1 && part.starts_with('0'))", "false"),
    ("manifest: size cap off", "manifest.rs", "if bytes.len() > MAX_MANIFEST_BYTES {", "if false {"),
    ("manifest: permissions allowed", "manifest.rs", "if !self.permissions.is_empty() {", "if false {"),
    ("manifest: hash not checked", "manifest.rs", "if !is_sha256_hex(&file.sha256) {", "if false {"),
    ("manifest: total size unchecked", "manifest.rs", "if total > limits.max_total {", "if false {"),
    ("manifest: plug-in without interface allowed", "manifest.rs", "(Kind::Plugin, None) => false,", "(Kind::Plugin, None) => true,"),
    ("manifest: url user-info allowed", "manifest.rs", " && !authority.contains('@')", ""),
    ("palette: contrast never fails", "palette.rs", "if contrast_ratio(front, back) < need {", "if contrast_ratio(front, back) < 0.0 {"),
    ("palette: dark flag unchecked", "palette.rs", "if self.dark != (self.background.luminance() < self.text.luminance()) {", "if false {"),
    ("palette: token names unchecked", "palette.rs", "if !tokens.keys().all(|name| valid_token_name(name)) {", "if false {"),
    ("index: expiry boundary", "index.rs", "if index.expires_at <= now {", "if index.expires_at < now {"),
    ("index: same version counted as rollback", "index.rs", "if index.version < newest_seen {", "if index.version <= newest_seen {"),
    ("index: duplicates allowed", "index.rs", "if !seen.insert((&entry.id, entry.version)) {", "if !seen.insert((&entry.id, entry.version)) && false {"),
    ("index: file tier allowed", "index.rs", "&& self.tier != Tier::FromFile", ""),
    ("index: revocation limit inclusive", "index.rs", "version < limit", "version <= limit"),
    ("tier: third party always admitted", "tier.rs", 'if cfg!(feature = "third-party") =>', "if true =>"),
    ("tier: reserved names open to everyone", "tier.rs", "if tier != Tier::FirstParty && id.in_first_party_namespace() {", "if false {"),
]

STORE = [
    ("unpack: archive size cap off", "unpack.rs", "if bytes.len() as u64 > caps.max_archive {", "if false {"),
    ("unpack: entry count cap off", "unpack.rs", "if zip.len() > MAX_ENTRIES {", "if false {"),
    ("unpack: names unchecked", "unpack.rs", "validate_path(name).map_err(UnpackError::BadName)?;", "let _ = validate_path(name);"),
    ("unpack: encrypted allowed", "unpack.rs", "if entry.encrypted() {", "if false {"),
    ("unpack: links allowed", "unpack.rs", "if entry.is_symlink() {", "if false {"),
    ("unpack: unlisted files allowed", "unpack.rs", "if files.iter().any(|(_, n)| n != MANIFEST_NAME && !listed.contains(n.as_str())) {", "if false {"),
    ("unpack: declared size unchecked", "unpack.rs", "if entry.size() != want.size {", "if false {"),
    ("unpack: overlong data tolerated", "unpack.rs", "if total > want.size {", "if false {"),
    ("unpack: short data tolerated", "unpack.rs", "if total != want.size {", "if false {"),
    ("unpack: hash unchecked", "unpack.rs", "if hex_digest(hasher) != want.sha256 {", "if false {"),
    ("unpack: ratio check off", "unpack.rs", "if entry.size() > RATIO_FLOOR && entry.size() / entry.compressed_size().max(1) > caps.max_ratio {", "if false {"),
    ("unpack: existing files overwritten", "unpack.rs", ".create_new(true)", ".create(true).truncate(true)"),
    ("store: tier not admitted", "store.rs", "self.policy.admit(tier).map_err(StoreError::Tier)?;", "let _ = self.policy.admit(tier);"),
    ("store: reserved names unchecked", "store.rs", "self.policy.check_id(tier, &manifest.id).map_err(StoreError::Tier)?;", "let _ = self.policy.check_id(tier, &manifest.id);"),
    ("store: theme unchecked", "store.rs", "ThemeFile::parse(&text).map_err(StoreError::BadTheme)?;", "let _ = ThemeFile::parse(&text);"),
    ("store: first-party replaced by a file", "store.rs", "if existing.tier == Tier::FirstParty && tier != Tier::FirstParty {", "if false {"),
    ("store: downgrade allowed", "store.rs", "if record.version < existing.version {", "if false {"),
    ("store: same version allowed", "store.rs", "if record.version == existing.version {", "if false {"),
    ("store: existing package folder overwritten", "store.rs", "if destination.exists() {", "if false {"),
    ("store: generation not bumped", "store.rs", "state.generation = state.generation.saturating_add(1);", "state.generation = state.generation;"),
    ("store: lock skipped", "store.rs", "let _lock = self.lock()?;", "let _lock = ();"),
    ("store: uninstall keeps theme choice", "store.rs", "state.active_theme.retain(|_, chosen| chosen != id);", ""),
    ("store: switching off keeps theme choice", "store.rs", "if state.active_theme.get(app) == Some(id) {", "if false {"),
    ("store: a non-theme can be chosen", "store.rs", "if existing.kind != Kind::Theme {", "if false {"),
    ("store: a switched-off theme can be chosen", "store.rs", "if !state.is_enabled(app, id) {", "if false {"),
    ("store: apps not matched (enable and choose)", "store.rs", "if !existing.apps.contains(app) {", "if false {", 2),
    ("reader: switched-off extensions listed", "reader.rs", "i.apps.contains(app) && state.is_enabled(app, &i.id))", "i.apps.contains(app))"),
    ("reader: theme not checked again", "reader.rs", "ThemeFile::parse(&bytes).ok()", "serde_json::from_slice::<ThemeFile>(&bytes).ok()"),
    ("reader: non-theme worn", "reader.rs", "if installed.kind != Kind::Theme || !state.is_enabled(app, id) {", "if !state.is_enabled(app, id) {"),
    ("reader: switched-off theme worn", "reader.rs", "if installed.kind != Kind::Theme || !state.is_enabled(app, id) {", "if installed.kind != Kind::Theme {"),
    ("location: override unchecked", "location.rs", "return if looks_absolute(os, &custom) {", "return if true {"),
    ("location: relative folders accepted", "location.rs", "env(name).filter(|v| looks_absolute(os, v))", "env(name)"),
    ("state: newer schema accepted", "state.rs", "if head.schema > SCHEMA {", "if false {"),
    ("state: size cap off", "state.rs", "if bytes.len() as u64 > MAX_STATE_BYTES {", "if false {"),
]

# Survivors that cannot be killed, with the reason. Anything else that survives fails the run.
EQUIVALENT = {
    "unpack: overlong data tolerated": "the archive reader never yields more bytes than the declared size and the read is bounded by size+1; kept as a backstop",
    "unpack: short data tolerated": "the archive reader reports truncated data as a read error and the hash check would refuse it anyway; kept as a backstop",
    "store: existing package folder overwritten": "Windows, and any non-empty folder, make the rename fail on their own; the check matters for an empty folder on Unix (covered by a test that only fails there)",
}

GROUPS = {
    "core": ("craft-ext-core", ROOT / "crates" / "ext-core" / "src", CORE),
    "store": ("craft-ext-store", ROOT / "crates" / "ext-store" / "src", STORE),
}


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 1 or args[0] not in GROUPS:
        print(__doc__)
        return 2
    package, src, mutants = GROUPS[args[0]]
    only = os.environ.get("ONLY")
    if only:
        mutants = [m for m in mutants if only in m[0]]
    originals = {}
    for m in mutants:
        label, name, old, expected = m[0], m[1], m[2], (m[4] if len(m) > 4 else 1)
        text = (src / name).read_text(encoding="utf-8")
        originals.setdefault(name, text)
        count = text.replace("\r\n", "\n").count(old)
        if count != expected:
            print(f"BAD PATTERN ({count} matches, wanted {expected}): {label}")
            return 2
    if "--dry" in sys.argv:
        print(f"OK: {len(mutants)} patterns match as expected")
        return 0
    survivors = []
    for m in mutants:
        label, name, old, new = m[0], m[1], m[2], m[3]
        path = src / name
        original = originals[name]
        try:
            path.write_text(original.replace("\r\n", "\n").replace(old, new), encoding="utf-8", newline="\n")
            run = subprocess.run(["cargo", "test", "-p", package, "-j", "8", "--lib"], cwd=ROOT, capture_output=True, text=True)
            out = run.stdout + run.stderr
            if "test result: FAILED" in out:
                print(f"caught    {label}", flush=True)
            elif "error[" in out or "could not compile" in out:
                print(f"NO BUILD  {label}", flush=True)
                survivors.append(label)
            elif label in EQUIVALENT:
                print(f"equivalent {label}: {EQUIVALENT[label]}", flush=True)
            else:
                print(f"SURVIVED  {label}", flush=True)
                survivors.append(label)
        finally:
            path.write_text(original, encoding="utf-8", newline="")
            os.utime(path, (time.time(), time.time()))
    print(f"{len(mutants) - len(survivors)} of {len(mutants)} caught")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
