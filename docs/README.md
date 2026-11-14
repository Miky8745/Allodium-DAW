# docs/

Consolidated, distilled knowledge for this project — filled in progressively,
stage by stage, as material from the two sibling reference repos
(`Allodium-tauri`, the original Tauri+Svelte app; `Allodium-GPUI-test`, the
validated GPUI prototype) actually gets used in the rewrite. Goal: by the
time the rewrite reaches feature parity with the MVP scope
(`daw-rewrite-starter/02-feature-scope.md`), everything relevant from those
two repos should have a home here, so they can be deleted without losing
anything this project still needs.

Deliberately **not** carried forward: anything specific to the old stack
that doesn't apply to a native GPUI app — the WebKitGTK/NVIDIA compositing
bug that motivated this rewrite, Svelte component structure, Tauri IPC
command wiring, the old CSS theme tokens. Those are dead context here.

What's in here so far:

- `gpui-api-notes.md` — GPUI API shape, event-handling patterns, the NixOS
  build-environment fix (`shell.nix` + the `xattr` crates-io patch, both
  already applied at the repo root), and performance lessons, all carried
  over from `Allodium-GPUI-test`'s prototype work.
- `stage-01-foundation.md` — what Stage 1 actually built and decided.

This is distinct from `PROGRESS.md` at the repo root, which is a terse
"what stage is active / what's done" status file, not a knowledge base.
