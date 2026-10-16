# Inspiration & positioning

CelestineMD is an **independent, open-source Markdown knowledge base** written in Rust. It is **inspired by** [Obsidian](https://obsidian.md) — local vaults, wikilinks, graph, backlinks, keyboard-first workflows — but it is **not** a clean-room rewrite, clone, or drop-in substitute for Obsidian’s product.

## What that means

- Celestine has its own UI, settings, themes, and feature set.
- Overlap with Obsidian ideas (plain Markdown folders, `[[wikilinks]]`, outline/backlinks/graph) is intentional inspiration, not parity.
- Opening an existing Markdown vault (including folders that also have a `.obsidian/` directory) is a convenience: Celestine uses the notes, not Obsidian’s app, plugins, or proprietary formats.
- Celestine is **not affiliated with** Obsidian or Dynalist Inc.

## Good practice (still)

Do not unpack or copy proprietary Obsidian application binaries, ASAR archives, Electron sources, or UI assets into this project. Build Celestine as its own software.
