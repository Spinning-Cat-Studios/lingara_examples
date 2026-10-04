# Changelog

## v0.1.0-alpha.1 — 2026-10-04


### Added

- `integrations/fake-game` (TypeScript): a headless game on the event feed —
  it sends `world.context_changed`, waits for its lesson plan's
  `lesson_plan.ready`, and receives webhooks.
- `integrations/luba-dive-deck` (Go): Luba's Dive Deck — a dispatch server
  and an ops dashboard on one client, each asking for only its own scope.
- `integrations/farducks-batter-rewards` (PHP): Farducks' Batter Rewards — a
  backend that creates a lesson plan while the order fries, reads it back by
  its id, and a runbook for the day its secret leaks.
- `apps/luba-dive-log` (TypeScript): Luba's Dive Log — the first app, on
  `@lingara/apps` and `@lingara/api`: its manifest, listing it, the install
  consent with every context slice optional, the card relay and an action.

### Changed

### Fixed

### Removed
