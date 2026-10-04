# The checks and the dispatch targets (ADR 4.10.26b D5).
#
# Workflows call the dispatch targets (`build-example DIR=<path>`), never
# `build-example-<language>`: the leak scan requires every `make` target in a
# shipped workflow to be one a shipped make file defines explicitly, and a
# matrix expression is not one. Each reads the row's language with
# `lingara-examples-check lang` and runs the recipe's own target. PKG reaches
# the sub-make through MAKEFLAGS. The variable is never called LANG, which is
# the locale.
#
# The dispatch targets and the loops run $(EXAMPLES_CHECK) as found and never
# build it, so `make help` on a cold clone builds nothing.

.PHONY: build-checker check-examples-index check-example-deps check-publishable \
        build-example test-example lock-example live-example \
        build-examples test-examples help-examples

build-checker:
	$(CARGO) build --release -p lingara-examples-check

check-examples-index: build-checker
	$(EXAMPLES_CHECK) index

check-example-deps: build-checker
	$(EXAMPLES_CHECK) deps

## scs-snapshot's precheck: Rust only, so it runs anywhere a publish does.
## Building examples is CI's (ci.yml, and staging's release.yml on the tag).
check-publishable: check-examples-index check-example-deps
	$(CARGO) test --workspace

define require-checker
	@test -x "$(EXAMPLES_CHECK)" || { echo "✗ $(EXAMPLES_CHECK) is missing: run make build-checker, or set EXAMPLES_CHECK" >&2; exit 2; }
endef

build-example test-example lock-example live-example:
	@test -n "$(DIR)" || { echo "Usage: make $@ DIR=<example path>" >&2; exit 2; }
	$(require-checker)
	@language=$$($(EXAMPLES_CHECK) lang "$(DIR)") && $(MAKE) --no-print-directory $@-$$language DIR="$(DIR)"

build-examples test-examples:
	$(require-checker)
	@$(EXAMPLES_CHECK) list | while read -r path language; do \
	  $(MAKE) --no-print-directory $(patsubst %s,%,$@) DIR="$$path" || exit 1; \
	done

help-examples:
	@echo ""
	@echo "Examples:"
	@echo "  make check-publishable                 - The index, every example's dependencies, the checker's tests"
	@echo "  make build-example test-example DIR=<path> - Build and test one example"
	@echo "  make build-examples / test-examples    - Every example in examples.toml"
	@echo "  make lock-example DIR=<path> PKG=<name>    - Regenerate one example's lock file"

HELP_SECTIONS += examples
