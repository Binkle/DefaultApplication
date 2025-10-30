# Repository Guidelines

This file is the concise contributor guide and agent instructions for this repository. Keep changes minimal, focused, and documented.

## Project Structure & Module Organization

- `src/` — application code (modules, services, CLI).
- `tests/` — automated tests mirroring `src/`.
- `scripts/` — dev utilities (setup, CI helpers).
- `assets/` — static files (images, fixtures).
- `docs/` — architecture notes and usage docs.
- `.github/` — workflows, issue/PR templates.

Create missing folders as needed. Keep modules small, single‑purpose, and named by domain (e.g., `src/users/`, `src/payments/`).

## Build, Test, and Development Commands

- Setup: `make setup` or `npm ci` or `pip install -r requirements.txt`.
- Lint: `make lint` or `npm run lint` or `ruff check .`.
- Format: `make format` or `npm run format` or `black .`.
- Type‑check: `make typecheck` or `npm run typecheck` or `mypy .`.
- Test: `make test` or `npm test` or `pytest -q`.
- Run locally: `make run` or `npm start` or `python -m src`.

## Coding Style & Naming Conventions

- Indentation: 2 spaces for JS/TS; 4 for Python.
- Names: `lower_snake_case` files; `CamelCase` classes; `lowerCamelCase` functions/vars.
- Keep functions small and pure; commit only formatted, lint‑clean code.

## Testing Guidelines

- Frameworks: Jest/Vitest (JS/TS) or Pytest (Python).
- Layout: mirror `src/` under `tests/`.
- Naming: `*.spec.ts` or `test_*.py`.
- Coverage: target 80%+ for changed code; add regression tests for bugs.

## Commit & Pull Request Guidelines

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`...).
- Small, focused commits with clear, imperative messages.
- PRs include: summary, linked issues (`Closes #123`), screenshots for UI, test evidence, and risk/rollback notes.

## Security & Configuration

- Do not commit secrets; use env vars and provide `.env.example`.
- Validate inputs; sanitize logs; apply least‑privilege configs.

## Agent‑Specific Instructions

- Obey this AGENTS.md; scope changes to the task.
- Favor simple, surgical patches; avoid broad refactors.
- Update docs/tests when code changes.
- Prefer `rg` to navigate; keep outputs concise; do not add licenses.

