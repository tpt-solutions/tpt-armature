# Changelog

All notable changes to `tpt-armature-gui` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Panel layout primitives (`Panel`, `PanelLayout`) available without a windowing backend.
- egui/eframe desktop application behind the `app` feature (three tiled views:
  Hex, Assembly, Graph) plus Strings and Pseudocode info panels.
- Graph view renders a scrollable node-and-edge canvas (one function at a time)
  with arrowheads; clicking a node jumps the Assembly view to that block.
- Assembly view shows clickable X-refs (incoming references and branch/call
  targets) for the selected instruction.
- Interactivity: goto-address, search, keyboard navigation.
- `scripts` feature: in-app Rhai script console.
- `wasm32` browser build mounting into the `#armature_canvas` element.

## [0.1.0]

### Added
- Workspace scaffolding (Phase 0): dual MIT/Apache-2.0 license, CI, lint, format.
