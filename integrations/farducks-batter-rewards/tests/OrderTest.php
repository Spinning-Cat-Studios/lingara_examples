<?php

declare(strict_types=1);

namespace Farducks\Tests;

use Farducks\Environment;
use Farducks\Order;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;

final class OrderTest extends TestCase
{
    public const PLAN_ID = '3f1c2a9e-5b7d-4e21-9a0c-6d8e4f2b1a37';

    public const PLAN = '{"id":"' . self::PLAN_ID . '","status":"complete","title":"At the fish and chip counter",'
        . '"source_lang":"en","target_lang":"es","level":2,"created_at":"2026-10-04T10:00:00Z",'
        . '"completed_at":"2026-10-04T10:00:41Z","ai_generated":true,'
        . '"content":{"learning_objectives":["Order at a counter"],"vocabulary":[],"sets":[]}}';

    public const TOKEN = '{"access_token":"lgr_at_farducks","token_type":"Bearer","expires_in":3600,'
        . '"scope":"lesson_plans:write lesson_plans:read"}';

    /** @return array<string, array{list<array{string, string}>}> */
    public static function streams(): array
    {
        return [
            'started' => [[
                ['started', '{"plan_id":"' . self::PLAN_ID . '"}'],
                ['phase', '{"phase":"selecting_vocabulary","attempt":1}'],
                ['result', '{"plan":' . self::PLAN . '}'],
            ]],
            'lone result' => [[
                ['result', '{"plan":' . self::PLAN . '}'],
            ]],
        ];
    }

    /** @param list<array{string, string}> $events */
    #[DataProvider('streams')]
    public function testCreatesThenReadsBack(array $events): void
    {
        $http = new RecordedHttp([
            RecordedHttp::json(200, self::TOKEN),
            RecordedHttp::sse($events),
            RecordedHttp::json(200, self::PLAN),
        ]);
        $env = Environment::from(['LINGARA_CLIENT_ID' => 'lgr_cid_farducks', 'LINGARA_CLIENT_SECRET' => 'lgr_cs_farducks']);
        $lines = [];
        $order = new Order($env->client($http->stack()), static function (string $line) use (&$lines): void {
            $lines[] = $line;
        });

        self::assertSame(0, $order->run());
        self::assertSame(
            ['POST /oauth/token', 'POST /v1/lesson-plans', 'GET /v1/lesson-plans/' . self::PLAN_ID],
            $http->routes(),
        );
        parse_str((string) $http->sent[0]->getBody(), $form);
        self::assertSame('lesson_plans:write lesson_plans:read', $form['scope'] ?? null);
        self::assertSame(['order: plan ' . self::PLAN_ID, 'read back: "At the fish and chip counter" (complete)'], $lines);
    }
}
