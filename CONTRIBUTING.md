# Contributing

Examples are welcome: a fix to one of ours, or one of your own.

## The rules every example keeps

- **Released packages only.** An example installs Lingara's packages from the
  public registry, at the exact version `examples.toml` names: never a range,
  a local path, a git URL, a workspace link or a private registry. The same
  holds for every third-party dependency, which a committed lock file pins.
  `make check-example-deps` checks all of this, and CI runs it on your pull
  request.
- **One language per example**, with a recipe in `make/lang/<language>.mk`.
- **One row per example in `examples.toml`.** `make check-examples-index`
  fails on a directory with no row, and on a README table that no longer
  matches `lingara-examples-check list --markdown`.
- **No secrets.** Credentials come from the environment; see `SECURITY.md`.

## How a pull request lands

This repository is published from our private repository, so a pull request
is not merged here directly. When we accept one, we re-land it through our
private repository with you credited as co-author, and it reaches this
repository with the next release. Your pull request is closed with a link to
that release.
