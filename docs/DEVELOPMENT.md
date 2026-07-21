# Development Guide

This document sets the working rules so DB-Eye development stays focused, traceable, and easy to review.

## Principles

- Every feature needs a clear purpose before coding starts.
- Every significant change must be recorded.
- Every release must have a changelog.
- Every major technical decision must have a stated reason.
- Production safety matters more than shipping features fast.

## Development Flow

### 1. Plan

Before implementing, write a short summary of:

- The problem being solved.
- Feature scope.
- Non-scope.
- Risks.
- Acceptance criteria.

Use the template:

- `docs/templates/FEATURE_SPEC.md`

### 2. Implement

While coding:

- Keep changes small and focused.
- Separate feature work, refactors, and docs where possible.
- Avoid manual SQL for new user input; prioritize parameterized queries.
- Update documentation alongside the feature change.

### 3. Validate

Minimum before committing:

```bash
cargo fmt
cargo check
cargo clippy
```

If related tests already exist:

```bash
cargo test
```

For database features, validate against SQLite first, then PostgreSQL/MySQL if the feature is cross-database.

### 4. Document

Every change must check the following docs:

- `README.md` — if user-facing behavior changed.
- `PRD.md` — if the roadmap/priorities changed.
- `CHANGELOG.md` — if the feature/fix/change is release-worthy.
- `docs/DEVLOG.md` — daily development notes/large changes.
- A new ADR — if there's a significant architecture decision.

### 5. Commit

Use Conventional Commits:

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation
- `refactor:` structural change with no new behavior
- `test:` add/change tests
- `chore:` maintenance

Example:

```text
feat: add parameterized row updates
fix: handle null values in CRUD forms
docs: update production roadmap
```

### 6. Release

Before releasing:

- Run the checklist in `docs/templates/RELEASE_CHECKLIST.md`.
- Make sure `CHANGELOG.md` is updated.
- Make sure the `Cargo.toml` version is correct.
- Create a git tag.

## Definition of Done

A task is considered done when:

- Acceptance criteria are met.
- `cargo fmt` succeeds.
- `cargo check` succeeds.
- `cargo clippy` has no new errors.
- Related tests are added/updated where possible.
- README/PRD/CHANGELOG/DEVLOG are updated where relevant.
- Changes are committed with a clear message.

## Technical Decision Documentation

For major decisions, create an ADR at:

```text
docs/adr/YYYY-MM-DD-short-title.md
```

Examples of decisions that need an ADR:

- Parameterized query strategy across SQLite/PostgreSQL/MySQL.
- How composite primary keys are supported.
- Saved connections config format.
- Read-only mode and safety guards.

ADR format:

```md
# ADR: Title

## Status
Accepted / Proposed / Superseded

## Context
The problem and constraints.

## Decision
The decision that was made.

## Consequences
Positive/negative impact.
```

## Current Development Priorities

Follow `PRD.md`:

1. P0 first, for production readiness.
2. P1 once safety and tests are solid.
3. P2 once the core is stable.
