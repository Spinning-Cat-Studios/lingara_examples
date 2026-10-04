# Shared variables (ADR 4.10.26b D5).
#
# EXAMPLES_CHECK is the checker binary `build-checker` writes. A CI leg that
# downloaded it sets EXAMPLES_CHECK to its path and never needs cargo.

HELP_SECTIONS :=

CARGO ?= cargo
EXAMPLES_CHECK ?= target/release/lingara-examples-check
