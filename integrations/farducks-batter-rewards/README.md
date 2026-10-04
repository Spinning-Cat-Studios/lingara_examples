# Farducks' Batter Rewards

**Package:** [`spinningcatstudios/lingara`](https://packagist.org/packages/spinningcatstudios/lingara),
at the version `composer.json` pins.
**Scopes:** `lesson_plans:write` and `lesson_plans:read`.

Farducks is a chain of fish and chip shops. Its loyalty app, Batter Rewards,
gives a customer a short lesson while their order fries. The app and the
tills never talk to Lingara and never hold the secret: they ask Farducks' own
backend, and this project stands in for that backend.

For one order it:

1. asks for both scopes in **one** token exchange (`scope` is
   space-separated on the wire), since one process both creates and reads;
2. creates the lesson plan (`POST /v1/lesson-plans`) and takes its id: a
   `started` event carries it, and a plan served from Lingara's pool arrives
   as a lone `result` that carries the whole plan;
3. reads the plan back by that id (`GET /v1/lesson-plans/{id}`), the step a
   till or a later request makes with nothing but the id, and prints its
   title and status.

## The client to create

One OAuth client, allowed `lesson_plans:write` and `lesson_plans:read`, on
allowance billing. Its secret lives on the backend and nowhere else.

## Running it

You need PHP 8.2 or later and Composer.

```sh
composer install
LINGARA_CLIENT_ID=… LINGARA_CLIENT_SECRET=… php bin/order.php
composer test      # offline: recorded responses stand in for Lingara
```

`LINGARA_BASE_URL` and `LINGARA_TOKEN_URL` override the production
endpoints. The HTTP stack is Symfony's client over Nyholm's PSR-7, the
library's recommended stack.

## The day it leaks

Someone pastes the client secret into a support ticket. Rotating it is a
console action plus a deploy, not code, so this is a runbook:

1. **Create a second secret** on the client in the Lingara console. Both
   secrets now work.
2. **Set `LINGARA_CLIENT_SECRET`** to the new secret and restart the backend.
   The code reads the secret from the environment and nowhere else, so a
   restart is the whole deploy.
3. **Watch the old secret's "Last used"** in the console. When it stops
   moving, nothing is using it any more.
4. **Revoke the old secret.** Lingara refuses its tokens at once, and a token
   exchange with it fails with `invalid_client`.

This project never prints the secret: `tests/SecretTest.php` checks that
nothing an order prints contains it, on success or when Lingara refuses the
exchange. The library keeps the secret and every token out of `var_dump`,
`print_r`, `var_export`, `json_encode` and its error messages, and proves it in
its own test,
[`php/tests/RedactionTest.php`](https://github.com/Spinning-Cat-Studios/lingara_api_clients/blob/main/php/tests/RedactionTest.php).
