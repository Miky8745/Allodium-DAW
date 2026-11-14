# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

The **GPUI rewrite** of Allodium, a DAW — a native Rust/GPUI desktop app.
Stage 1 (foundation) is complete: a scaffolded `cargo` project with a theme
module and an empty three-zone app shell. See `PROGRESS.md` for current
stage status and `docs/stage-01-foundation.md` for what Stage 1 built.

## Commands

- Build/run (always via `nix-shell`, not bare `cargo` — see
  `docs/gpui-api-notes.md` for why): `nix-shell --run "cargo build"` /
  `nix-shell --run "cargo run"`.
- Clean builds take several minutes (GPUI's dependency tree is large);
  incremental rebuilds are much faster.

## Sibling repos

Two related repos live under `~/Programming/` as reference material this
rewrite draws from. As an ongoing practice (not a one-time step),
relevant knowledge from both is being folded into `docs/` as it's actually
used, so these can eventually be deleted without losing anything this
project needs. If a session can't find either on this machine, ask the
user rather than silently re-deriving their content from the planning
docs alone.

- **`Allodium-tauri`** — the original Tauri + SvelteKit DAW. Its
  `docs/*.md` are the authoritative behavior spec (piano roll pixel math,
  audio scheduling, playlist/mixer semantics) — read the relevant doc
  there before implementing a feature here. Its `src-tauri/src/` Rust
  backend (`project.rs` especially) is directly portable logic.
- **`Allodium-GPUI-test`** — the validated GPUI prototype that proved this
  rewrite is worth doing (see `docs/gpui-api-notes.md` for the distilled
  API/environment knowledge already pulled from it). Its `src/grid.rs`,
  `src/model.rs`, `src/audio.rs` are meant to be ported forward during the
  Piano Roll and audio-engine stages, not re-derived.

## Starting point for a new session

Read, in this order, before writing any code:

1. `daw-rewrite-starter/00-brief.md` — the task and ground rules.
2. `daw-rewrite-starter/01-theme.md` — visual design tokens (already
   captured as real constants in `src/theme.rs`).
3. `daw-rewrite-starter/02-feature-scope.md` — the **MVP feature list**.
   Defines exactly what "basic functionality" means and what's explicitly
   deferred. Do not build anything from the stretch list before the MVP
   list is fully done end-to-end — note deferred ideas in `PROGRESS.md`
   instead of acting on them.
4. `daw-rewrite-starter/03-reuse-map.md` — what to copy from the sibling
   repos vs. design fresh. Check before writing any non-trivial logic.
5. `daw-rewrite-starter/stages/01-foundation.md` through `08-integration-
   and-polish.md` — one stage per file, worked **in order**, each with its
   own "done when" checklist. Only load the stage currently being worked.

## Working conventions

- **Keep `PROGRESS.md`** at this repo's root up to date — active stage,
  what's done, deferred ideas. A session resuming later should be able to
  read just `PROGRESS.md` + the current stage file and know where to pick
  up.
- **Keep `docs/` up to date** — see `docs/README.md`. When a session pulls
  real architectural knowledge from a sibling repo (GPUI API shape, a
  ported algorithm, a behavior spec) or makes a non-obvious decision,
  write it to its own `docs/<topic>.md` file and link it from here, rather
  than inlining it into this file.
- Design tokens belong in `src/theme.rs` as real constants — never
  hardcode a color/radius/font inline in a component.
- Do not put `eprintln!`/`println!` inside per-event input handlers — see
  `docs/gpui-api-notes.md`'s performance-lessons section for why.
- Periodically re-run the same stress test that validated the prototype
  (scroll + drag + delete while audio plays, watching for steady fps and
  no audio glitches) as real UI and audio land — not just once at the end.
