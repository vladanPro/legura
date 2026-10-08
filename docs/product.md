# Product Boundary

Date: 2026-10-08

## Layers

- Axonyx supplies parsing/lowering, routes, data functions, actions, state,
  database adapters, sessions, validation, uploads, and deployment tooling.
- Axonyx UI supplies Foundry components and reusable presentation contracts.
- Legura owns setup, administration, content models, publishing, editor,
  media management, roles, theme selection, and product-specific workflows.

General framework defects are fixed upstream, not hidden with CMS-only patches.
Legura uses published dependencies where possible, without copying runtime or
UI implementation into this repository.

## User Choices

Users may install Legura as a product without knowing Rust or Axonyx.
Developers may customize Legura through documented extension boundaries.
Developers may also use Axonyx to build an unrelated CMS or website from scratch.
None of these paths replaces or restricts the others.

## Initial Content Model

Posts have a title, unique slug, body, draft/published status, and timestamps.
The public site serves published posts only. Drafts and administration require
authorization. The first editor is intentionally simple; rich text requires a
defined sanitization/rendering policy before it can be public.

## Explicit Non-Goals For V0

- WordPress plugin, theme, or database compatibility.
- Arbitrary execution of uploaded plugins or server code.
- Visual page builder, theme marketplace, or hosted SaaS platform.
- Multi-tenant administration, social login, or complex role hierarchies.
- Unmeasured performance or security superiority claims.

## Naming

Legura supersedes Blockbit as the product name for this new implementation.
Existing Blockbit repositories and historical notes remain untouched unless
their migration is explicitly planned. Repository creation does not publish or
reserve a package name, domain, or trademark.
