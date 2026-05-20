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

## Project conventions

- User manages all Zellij config changes (`config.kdl`) — do not edit it.
- `just install` or `just dev` to build and install the WASM for local testing.
- Versioning follows semver; bump the patch version (`Cargo.toml`) on bugfix releases.
