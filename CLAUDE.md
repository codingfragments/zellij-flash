# zellij-flash — Claude Code conventions

## Gitflow

- **Always create a branch before making any change or commit** — even one-line fixes.
- Branch prefixes:
  - `bug/` — bug fixes
  - `feature/` — new features
  - `phase/` — larger milestone / multi-commit work
- If the type is unclear, ask before creating the branch.
- Every branch lands via PR to `main` — no direct commits to `main`.
- Stay on the working branch until the PR is explicitly merged; switch back to `main` only after merge.

## Workflow

- Commit frequently within a branch as work progresses.
- Do not push without user approval — summarise what's testable and wait for "push" / "looks good".
- End each phase with: what to test, how to trigger it, what works vs what's still a stub.

## Release process

Releases are always a **two-step merge flow** — never tag directly from a feature/bug branch:

1. **Code PRs** (`bug/`, `feature/`, `phase/`) — contain only the code changes; merge these first.
2. **Release PR** (`bug/release-x.y.z` or `feature/release-x.y.z`) — a separate branch and PR that contains:
   - `Cargo.toml` version bump (semver: patch for bugs, minor for features, major for breaking)
   - `CHANGELOG.md` entry summarising what changed
   - Any other release-specific housekeeping
3. Merge the release PR to `main`.
4. Tag the resulting merge commit: `git tag v<x.y.z> && git push origin v<x.y.z>`
5. Pushing the tag triggers `.github/workflows/release.yml`, which builds the wasm artifact, computes SHA-256, and publishes the GitHub release automatically.

> Never push a `v*.*.*` tag from a feature branch or before the release PR is merged.

## Project conventions

- User manages all Zellij config changes (`config.kdl`) — do not edit it.
- `just install` or `just dev` to build and install the WASM for local testing.
- Versioning follows semver; bump the patch version (`Cargo.toml`) on bugfix releases.
