<p align="center">
  <img src="assets/brand/wordmark.svg" alt="Allodium" width="420">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-pre--alpha-red" alt="status">
  <img src="https://img.shields.io/badge/language-Rust-orange?logo=rust&logoColor=white" alt="language">
  <img src="https://img.shields.io/badge/UI-GPUI-blueviolet" alt="ui">
  <img src="https://img.shields.io/badge/platform-NixOS-5277C3?logo=nixos&logoColor=white" alt="platform">
  <img src="https://img.shields.io/badge/license-TBD-lightgrey" alt="license">
</p>

<p align="center">
  A native, GPU-accelerated DAW, built so scrolling, dragging, and
  deleting notes stays glitch-free while audio is playing, not just in a demo.
</p>

---

### About

Allodium is a digital audio workstation (channel rack, piano roll,
playlist, mixer) built from scratch as a native app rather than a
browser-based one. The point of that choice is interaction, not aesthetics:
a DAW lives or dies on whether a dense piano roll stays responsive while
you're mid-drag with audio still playing, and that's the bar this project
is actually being built against, from the first line of code rather than
as a later optimization pass.

### Stack

| | |
|---|---|
| **Language** | Rust |
| **UI** | [GPUI](https://www.gpui.rs/) |
| **Audio** | [cpal](https://github.com/RustAudio/cpal) |
| **Platform** | NixOS, Vulkan-capable GPU |

### Build

```sh
nix-shell --run "cargo run"
```
