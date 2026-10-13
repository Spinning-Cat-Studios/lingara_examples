# Changelog

## v0.1.0-alpha.2 — 2026-10-13


### Added

- `games/bevy/night-market`: the Night Market in Bevy — walk a street, talk to two stall-keepers who answer at your level, practise, and earn a lantern when the result comes back — with its Rust server beside it, on `bevy_lingara`, `lingara-embed` and `lingara` from crates.io. `games/` opens with it.
- `lingara-examples-check` reads Cargo: the `crates` registry, a reader that holds every manifest of an example and its lock to crates.io, the sparse-index lookup, and a bump that rewrites each manifest declaring the crate.
- `games/jvm/night-market`: the same Night Market in Kotlin, a libGDX desktop game with its Kotlin server beside it, on `lingara-embed-kotlin`, `lingara-embed-natives-desktop`, `lingara-embed-server-kotlin` and `lingara-kotlin` from Maven Central. One Gradle build, its wrapper committed, each module locked.
- `lingara-examples-check` reads Gradle: the `maven` registry, a reader that holds a version catalogue, its settings, build files and lockfiles to Maven Central and exact pins, the `maven-metadata.xml` lookup, and a bump that rewrites one catalogue version.

### Changed

- `lingara-examples-check outdated` names on stderr every version it skips because it is not SemVer, for every registry.

### Fixed

### Removed

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
