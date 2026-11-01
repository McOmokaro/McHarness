---
title: Design System
sidebar_label: Design System
sidebar_position: 6
slug: /rea-integration
description: The McHarness design language — pitch-black, Apple-inspired, anti-slop.
---

# Design System

McHarness looks like it works: pitch-black, high-contrast, and unapologetically Apple. This page documents the design language applied to this documentation site and how it is enforced.

## The look

| Token | Value |
|---|---|
| **Background** | `#000000` |
| **Text** | `#f5f5f7` |
| **Accent** | `#2997ff` (Apple system blue) |
| **Font stack** | `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif` |
| **Heading weight** | `800` |
| **Heading tracking** | `-0.03em` to `-0.045em` (tight) |
| **Radius** | `0` |
| **Dividers** | `#1d1d1f` |

Dark mode is **enforced**, not suggested: the palette is declared on `:root` and on both `data-theme` values, and the color mode is locked in `docusaurus.config.js`.

## Anti-slop directives

Three rules keep the design honest:

- **No gradients.** Gradients are forbidden (`background-image: none`); every surface is flat colour.
- **No excessive radius.** Every component is squared off (`border-radius: 0`).
- **No centered prose.** Long paragraphs are always left-aligned.

## How it is enforced

Theming runs as a small, repeatable pipeline:

1. **Extract** — the design intelligence is captured to `apple-design.json`.
2. **Map** — the tokens become CSS custom properties.
3. **Override** — every `--ifm-` variable is set in `src/css/custom.css`.
4. **Swizzle** — the Navbar and Footer are ejected into `src/theme/`.
5. **Strip** — the default Docusaurus logo, search box, links, and text are removed for a minimal, wordmark-led layout.
6. **Build and deploy** — `npm run build`, then the push to `main` publishes to GitHub Pages.

:::note
The design system is not decoration — it is the visual half of the **Whole** idea. The site is as quiet and seam-free as the harness it documents.
:::

## Verify

```bash
npm run build
```

A green build confirms the design system is intact. The live site is published from `main` by the deployment workflow.
