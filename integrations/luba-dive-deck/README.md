# Luba's Dive Deck

**Package:** [`github.com/Spinning-Cat-Studios/lingara_api_clients/go`](https://pkg.go.dev/github.com/Spinning-Cat-Studios/lingara_api_clients/go),
at the version `go.mod` requires.
**Scopes:** `vocab:generate` (the dispatch server) and `usage:read` (the ops
dashboard).

Luba runs dive trips. Before each dive, the boat's cabin screens teach the
divers a few words in the language of the place they are diving. Two of her
processes talk to Lingara, and each asks for **only the scope it needs**:

| Command | Scope it asks for | What it calls | What it prints |
|---|---|---|---|
| `cmd/dispatch` | `vocab:generate` | `GenerateVocabulary` (`POST /v1/vocab/stream`) | each word as the cabin screen shows it |
| `cmd/dashboard` | `usage:read` | `GetUsage` (`GET /v1/usage`) | the allowance left |

Each command makes its call, prints, and exits, the way a dispatch job or a
dashboard refresh would be triggered.

## The client to create

One OAuth client, allowed `vocab:generate` and `usage:read`, on allowance
billing. Both processes use it, and each asks for one of its scopes.

## Running it

You need Go 1.23 or later.

```sh
export LINGARA_CLIENT_ID=… LINGARA_CLIENT_SECRET=…
go run ./cmd/dashboard
go run ./cmd/dispatch --dive jeju
go test ./...      # offline: an httptest server stands in for Lingara
```

`LINGARA_BASE_URL` and `LINGARA_TOKEN_URL` override the production
endpoints.

`--dive` picks the destination, and the destination picks the language:

| `--dive` | Language |
|---|---|
| `okinawa` (default) | `ja` |
| `jeju` | `ko` |
| `cozumel` | `es` |

`--source-lang` (default `en`), `--level` (default `2`) and `--count`
(default `5`) set the rest of the request.

## Why one scope each

Lingara does not enforce least privilege across your own processes. Had the
dashboard asked for `vocab:generate` too, it would have got it, because the
client holds it. Asking for less is your discipline, and it pays off on the
day a token leaks: a dashboard token that leaks can read usage, and nothing
else.

Asking for a scope the client does not hold is `invalid_scope` for the whole
token exchange. Lingara never silently narrows the request.

The offline tests prove each command's token request carries exactly its own
`scope`.
