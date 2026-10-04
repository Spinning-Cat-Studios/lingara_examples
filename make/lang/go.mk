# Go examples (ADR 4.10.26b D5).
#
# `live-example-go` is reached only through `make live-example` (ADR 4.10.26c
# D7): each of the example's commands once against production with the live
# job's credentials, each bounded by `timeout 300` (TIMEOUT=gtimeout on macOS).

TIMEOUT ?= timeout

.PHONY: build-example-go test-example-go lock-example-go live-example-go

build-example-go:
	cd "$(DIR)" && go build ./... && go vet ./...

test-example-go:
	cd "$(DIR)" && go test ./...

lock-example-go:
	cd "$(DIR)" && go mod tidy

live-example-go: build-example-go
	@test -n "$$LINGARA_EXAMPLES_CLIENT_ID" && test -n "$$LINGARA_EXAMPLES_CLIENT_SECRET" || { echo "✗ set LINGARA_EXAMPLES_CLIENT_ID and LINGARA_EXAMPLES_CLIENT_SECRET" >&2; exit 2; }
	cd "$(DIR)" && export LINGARA_CLIENT_ID="$$LINGARA_EXAMPLES_CLIENT_ID" LINGARA_CLIENT_SECRET="$$LINGARA_EXAMPLES_CLIENT_SECRET" && \
	  $(TIMEOUT) 300 go run ./cmd/dashboard && $(TIMEOUT) 300 go run ./cmd/dispatch
