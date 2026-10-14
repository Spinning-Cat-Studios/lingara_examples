# The websites section (ADR 14.10.26p D5).
#
# `site-example` runs one website's server in the foreground. It belongs to
# this section, not to make/examples.mk, which holds the verbs every row
# answers; a later websites/<stack> child extends it here. It runs the
# example's own Cargo workspace, where its rust-toolchain.toml picks the
# channel. Never `-p` (see make/lang/rust.mk).
#
# The server reads LINGARA_CLIENT_ID, LINGARA_CLIENT_SECRET,
# LINGARA_WEBHOOK_SECRET, LINGARA_BASE_URL, LINGARA_TOKEN_URL,
# NIGHT_MARKET_ALLOWED_ORIGIN and LINGARA_APP_ORIGIN from the environment,
# which make passes through unchanged.

.PHONY: site-example help-websites

site-example:
	@test -n "$(DIR)" || { echo "Usage: make site-example DIR=websites/<stack>/<example>" >&2; exit 2; }
	cd "$(DIR)/server" && $(CARGO) run --locked

help-websites:
	@echo ""
	@echo "Websites:"
	@echo "  make site-example DIR=<path>           - Serve a website on its registered and its unregistered origin"

HELP_SECTIONS += websites
