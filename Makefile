# Lingara examples
# ================
#
# The files included below are the public build: they ship. Tooling that
# names private repositories or checkouts is pulled in by ONE optional include
# near the end, so a checkout without it still builds, tests and helps.
#
# The language recipes are a `-include` of a glob (ADR 4.10.26b D5): the leak
# scan requires a hard `include` to name a literal shipped file, and a glob is
# not one. A missing recipe is caught by `lingara-examples-check index`
# instead, which requires make/lang/<language>.mk for every row's language.

include make/config.mk
-include $(sort $(wildcard make/lang/*.mk))
include make/examples.mk
-include make/private.mk

# Staging carries its own release targets in make/staging.mk, which is copied
# to staging and never promoted. The private checkout sets PRIVATE_MAKE_LOADED
# instead, so there this file is never read.
ifndef PRIVATE_MAKE_LOADED
-include make/staging.mk
endif

.DEFAULT_GOAL := help

# Each included file appends its section to HELP_SECTIONS, so an absent
# optional file drops its section instead of failing `help`.
.PHONY: help
help:
	@echo "Lingara examples"
	@echo "================"
	@for section in $(HELP_SECTIONS); do $(MAKE) -s help-$$section || exit 1; done
