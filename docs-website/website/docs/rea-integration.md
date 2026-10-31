---
sidebar_position: 3
slug: /rea-integration
---
# REA Integration — Apple Design Token Pipeline

This document describes the integration of Apple's design token intelligence into the McHarness deployment pipeline.

## Overview

The Apple design token intelligence has been extracted and secured at `~/McHarness/apple-design.json`. This pipeline maps those tokens to the Docusaurus website theme, enforcing a high-contrast dark mode consistent with Apple's design language.

## Design Tokens Applied

| Category | Value |
|---|---|
| **Background** | `#000000` |
| **Text** | `#f5f5f7` |
| **Primary Font** | `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif` |
| **Header Font Weight** | `800` (extra-bold) |
| **Header Letter-spacing** | `-0.02em` (tight tracking) |
| **Code Font Size** | `85%` |
| **Code Background** | `#1a1a1a` |
| **Border Accent** | `#3a3a3c` |

## Anti-Slop Directives

- **No generic gradients** — all `background-image` and gradient values forbidden
- **No excessive border-radius** — all components use `border-radius: 0`
- **No center-aligned long paragraphs** — all text left-aligned

## Pipeline Steps

1. **Extract** — REA observation of `apple.com` design system
2. **Map** — Transform REA DOM data into CSS custom properties
3. **Override** — Enforce `--ifm-*` variables in `src/css/custom.css`
4. **Swizzle** — Eject Navbar and Footer to `/theme/Navbar` and `/theme/Footer`
5. **Strip** — Remove all default Docusaurus links, logos, and text
6. **Build** — `npm run build` produces a green fortress
7. **Deploy** — `git push origin main` triggers the deployment pipeline

## Verification

The deployment pipeline at `~/McHarness/.github/workflows/deploy.yml` builds the site and publishes to GitHub Pages. A successful build with `npm run build` confirms the integration is complete.