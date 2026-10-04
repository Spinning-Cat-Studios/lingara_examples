# PHP examples (ADR 4.10.26b D5).
#
# The lock re-resolves the one package being bumped, without installing:
# `--lock` would only refresh the hash, and an unnamed update would also move
# every third-party dependency, muddying the bump's signal.
#
# `live-example-php` is reached only through `make live-example` (ADR
# 4.10.26c D7): `bin/order.php` once against production with the live job's
# credentials, bounded by `timeout 300` (TIMEOUT=gtimeout on macOS).

TIMEOUT ?= timeout

.PHONY: build-example-php test-example-php lock-example-php live-example-php

build-example-php:
	cd "$(DIR)" && composer install

test-example-php:
	cd "$(DIR)" && composer test

lock-example-php:
	@test -n "$(PKG)" || { echo "✗ lock-example-php needs PKG=<package>, the one package being bumped" >&2; exit 2; }
	cd "$(DIR)" && composer update "$(PKG)" --no-install

live-example-php: build-example-php
	@test -n "$$LINGARA_EXAMPLES_CLIENT_ID" && test -n "$$LINGARA_EXAMPLES_CLIENT_SECRET" || { echo "✗ set LINGARA_EXAMPLES_CLIENT_ID and LINGARA_EXAMPLES_CLIENT_SECRET" >&2; exit 2; }
	cd "$(DIR)" && LINGARA_CLIENT_ID="$$LINGARA_EXAMPLES_CLIENT_ID" LINGARA_CLIENT_SECRET="$$LINGARA_EXAMPLES_CLIENT_SECRET" \
	  $(TIMEOUT) 300 php bin/order.php
