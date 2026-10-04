# TypeScript examples (ADR 4.10.26b D5). Node runs them on its built-in type
# stripping, so each example carries a .node-version (≥ 22.18), which every
# workflow's setup-node step reads.
#
# `live-example-typescript` is reached only through `make live-example`
# (ADR 4.10.26c D7). It runs the example once against production with the
# live job's credentials, bounded by `timeout 300` (coreutils' `gtimeout` on
# macOS: TIMEOUT=gtimeout). The scene carries the date and the run id, because
# a fixed scene is served from the pool from its second day and the game then
# exits 1 by design.

TIMEOUT ?= timeout

.PHONY: build-example-typescript test-example-typescript lock-example-typescript live-example-typescript

build-example-typescript:
	cd "$(DIR)" && npm ci && npx tsc --noEmit

test-example-typescript:
	cd "$(DIR)" && npm test

lock-example-typescript:
	cd "$(DIR)" && npm install --package-lock-only

live-example-typescript: build-example-typescript
	@test -n "$$LINGARA_EXAMPLES_CLIENT_ID" && test -n "$$LINGARA_EXAMPLES_CLIENT_SECRET" || { echo "✗ set LINGARA_EXAMPLES_CLIENT_ID and LINGARA_EXAMPLES_CLIENT_SECRET" >&2; exit 2; }
	cd "$(DIR)" && LINGARA_CLIENT_ID="$$LINGARA_EXAMPLES_CLIENT_ID" LINGARA_CLIENT_SECRET="$$LINGARA_EXAMPLES_CLIENT_SECRET" \
	  $(TIMEOUT) 300 npm start -- --scene "A night market after rain, $$(date -u +%F) run $${GITHUB_RUN_ID:-local}"
