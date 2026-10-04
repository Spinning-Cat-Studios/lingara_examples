<?php

declare(strict_types=1);

namespace Farducks;

use Lingara\Client;
use Lingara\HttpStack;

/**
 * The environment every example in this repository reads, so a developer
 * sets it once. The secret is read from here and nowhere else, which is what
 * makes rotating it a restart: set LINGARA_CLIENT_SECRET to the new one.
 */
final readonly class Environment
{
    private function __construct(
        private string $clientId,
        #[\SensitiveParameter]
        private string $clientSecret,
        private ?string $baseUrl,
        private ?string $tokenUrl,
    ) {
    }

    public static function fromProcess(): self
    {
        return self::from(getenv());
    }

    /**
     * LINGARA_CLIENT_ID and LINGARA_CLIENT_SECRET are required;
     * LINGARA_BASE_URL and LINGARA_TOKEN_URL override production.
     *
     * @param array<string, string> $env
     */
    public static function from(#[\SensitiveParameter] array $env): self
    {
        $id = $env['LINGARA_CLIENT_ID'] ?? '';
        $secret = $env['LINGARA_CLIENT_SECRET'] ?? '';
        if ($id === '' || $secret === '') {
            throw new \RuntimeException('set LINGARA_CLIENT_ID and LINGARA_CLIENT_SECRET');
        }
        $base = ($env['LINGARA_BASE_URL'] ?? '') ?: null;
        $token = ($env['LINGARA_TOKEN_URL'] ?? '') ?: null;

        return new self($id, $secret, $base, $token);
    }

    /**
     * One process both creates and reads plans, so it asks for both scopes in
     * one exchange, space-separated on the wire. `$http` is the seam the
     * tests replace; production finds Symfony's client.
     */
    public function client(?HttpStack $http = null): Client
    {
        return new Client(
            clientId: $this->clientId,
            clientSecret: $this->clientSecret,
            scopes: ['lesson_plans:write', 'lesson_plans:read'],
            baseUrl: $this->baseUrl ?? Client::DEFAULT_BASE_URL,
            tokenUrl: $this->tokenUrl ?? Client::DEFAULT_TOKEN_URL,
            http: $http,
        );
    }
}
