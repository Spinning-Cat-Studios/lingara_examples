# Rust examples (ADR 13.10.26h D6). A Rust example is two Cargo workspaces of
# its own, `game/` and `server/`, each with its own Cargo.lock, so every target
# runs once per workspace in that directory, where the example's
# rust-toolchain.toml picks the channel. Never `-p`: the leak scan reads every
# `cargo … -p <crate>` in a shipped make file as a claim that the crate is a
# workspace member this repository ships, and no example is one.
#
# No `live-example-rust`: a game needs a player at the keyboard, so a Rust row
# is `live = false` (roadmap 13.10.26g D3).

RUST_HALVES := game server

.PHONY: build-example-rust test-example-rust lock-example-rust

build-example-rust:
	@for half in $(RUST_HALVES); do \
	  (cd "$(DIR)/$$half" && cargo build --locked --all-targets) || exit 1; \
	done

test-example-rust:
	@for half in $(RUST_HALVES); do \
	  (cd "$(DIR)/$$half" && cargo test --locked) || exit 1; \
	done

# After `bump` wrote the new `=` pin: re-lock each workspace whose manifest
# names PKG. The package is positional, not `-p` (see above).
lock-example-rust:
	@test -n "$(PKG)" || { echo "Usage: make lock-example DIR=<example path> PKG=<crate>" >&2; exit 2; }
	@for half in $(RUST_HALVES); do \
	  if grep -Eq '^[[:space:]]*"?$(PKG)"?[[:space:]]*[=.]' "$(DIR)/$$half/Cargo.toml"; then \
	    (cd "$(DIR)/$$half" && cargo update "$(PKG)") || exit 1; \
	  fi; \
	done
