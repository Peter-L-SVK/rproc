---
name: release
description: Cut a new release of a Cargo project — bump the version in Cargo.toml (major / minor / patch), refresh Cargo.lock, commit, create an annotated `vX.Y.Z` tag, and push branch + tag to `origin`. Use when the user asks to "release", "ship a new version", "bump the version", "cut a release", "tag a release", or any equivalent phrasing on a Rust/Cargo project.
version: 0.1.0
---

# Release a new Cargo version

You are driving a release of a Cargo project. The goal: bump `Cargo.toml`, refresh `Cargo.lock`, commit, tag, and push branch + tag — without surprising the user with any irreversible action.

Every step that touches the remote (`git push`) MUST be confirmed by the user before running. Local steps (editing files, commits, local tag) can proceed once the user has picked the bump type.

## Step 1 — Pre-flight checks

Run these in parallel and inspect the results before going further:

```bash
git rev-parse --abbrev-ref HEAD
git status --porcelain
git remote -v
```

- **Branch**: the default release branch is `main`. If the current branch is anything else, stop and ask the user whether to switch or release from the current branch.
- **Working tree**: must be clean (`git status --porcelain` empty). If not, stop and ask the user how to handle the uncommitted changes — never auto-stash or auto-commit them.
- **Remote**: there must be a remote named `origin`. If not, ask the user which remote to push to.

Also read `Cargo.toml` and extract the current `version = "X.Y.Z"` field. If the file isn't at the repo root or the version line can't be parsed, stop and tell the user.

## Step 2 — Ask which part to bump

Use `AskUserQuestion` with a single question, header "Bump", and three options. In each option's description, show the resulting version computed from the current one — that's what the user actually needs to compare.

Example, if the current version is `0.4.2`:

- **patch** → `0.4.3` (bug fixes only)
- **minor** → `0.5.0` (backwards-compatible features)
- **major** → `1.0.0` (breaking changes — for `0.x` versions, bumping `x` is also a breaking change; flag this in the description)

Recommend `patch` first (most common). Compute the new version yourself from the parsed semver — don't ask the user to type it.

## Step 3 — Apply the bump locally

1. **Edit `Cargo.toml`**: replace the `version = "..."` line in the `[package]` section. Use `Edit`, not `Write`, so the rest of the file is untouched.

2. **Refresh `Cargo.lock`**: run `cargo check` (faster than `cargo build`; it still updates the lock file). If the project doesn't compile, abort, surface the error, and leave `Cargo.toml` reverted only if the user asks — otherwise let them inspect.

3. **Show the diff** with `git diff Cargo.toml Cargo.lock` and confirm with the user that this is what they want to ship. This is the last chance to back out cheaply.

## Step 4 — Commit + tag locally

After the user confirms the diff:

```bash
git add Cargo.toml Cargo.lock
git commit -m "release vX.Y.Z"
git tag -a vX.Y.Z -m "Release vX.Y.Z"
```

- Use the exact tag format `vX.Y.Z` (lowercase `v`, no extra prefix).
- The tag is annotated (`-a`), not lightweight — annotated tags carry author/date metadata and show up properly in `git describe`.
- If the tag already exists locally (`git rev-parse vX.Y.Z` succeeds), stop and tell the user. Never overwrite an existing tag.
- If the commit fails (pre-commit hook), fix the underlying issue and create a NEW commit. Do not `--amend`.

## Step 5 — Push (requires confirmation)

Show the user exactly what you're about to push, then ask for explicit go-ahead. Don't push without confirmation.

```bash
git push origin <branch>
git push origin vX.Y.Z
```

Push the branch first, then the tag — if the branch push fails (e.g. non-fast-forward), the tag is still local and easy to clean up.

Never use `--force` or `--force-with-lease` here. If the branch push is rejected, stop and surface the error.

## Step 6 — Wrap-up

Report:
- The new version (`vX.Y.Z`)
- The commit hash of the release commit
- The fact that the tag was pushed to `origin`
- Any next steps the user mentioned (changelog, GitHub release notes, crates.io publish) — but do NOT run `cargo publish` yourself unless explicitly asked.

## Hard constraints

- Never amend, force-push, or rewrite an existing tag.
- Never run `cargo publish` unless the user explicitly asks.
- Never skip pre-commit hooks (`--no-verify`).
- Never auto-resolve a dirty working tree by stashing/committing — always ask.
- The remote-visible actions (`git push`) require user confirmation each time; a confirmation for the local bump is NOT a confirmation to push.