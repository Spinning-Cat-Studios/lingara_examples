<?php

declare(strict_types=1);

namespace Farducks\Tests;

use Farducks\Environment;
use Farducks\Order;
use PHPUnit\Framework\TestCase;

/**
 * What this example claims about its secret: it is required, and nothing the
 * order prints contains it. That the library keeps it out of var_dump and its
 * own messages is the library's claim, tested in the library.
 */
final class SecretTest extends TestCase
{
    private const SECRET = 'lgr_cs_farducksSupportTicketLeak0000000000000000';

    public function testSecretIsRequiredAndNeverPrinted(): void
    {
        try {
            Environment::from(['LINGARA_CLIENT_ID' => 'lgr_cid_farducks']);
            self::fail('Environment started without LINGARA_CLIENT_SECRET');
        } catch (\RuntimeException $e) {
            self::assertStringContainsString('LINGARA_CLIENT_SECRET', $e->getMessage());
        }

        $success = $this->printed([
            RecordedHttp::json(200, OrderTest::TOKEN),
            RecordedHttp::sse([['result', '{"plan":' . OrderTest::PLAN . '}']]),
            RecordedHttp::json(200, OrderTest::PLAN),
        ], 0);
        $refused = $this->printed([
            RecordedHttp::json(401, '{"error":"invalid_client","error_description":"client authentication failed"}'),
        ], 1);

        foreach ([$success, $refused] as $output) {
            self::assertStringNotContainsString(self::SECRET, $output);
        }
        self::assertStringContainsString('order failed', $refused);
    }

    /** @param list<\Psr\Http\Message\ResponseInterface> $responses */
    private function printed(array $responses, int $exit): string
    {
        $http = new RecordedHttp($responses);
        $env = Environment::from(['LINGARA_CLIENT_ID' => 'lgr_cid_farducks', 'LINGARA_CLIENT_SECRET' => self::SECRET]);
        $output = '';
        $order = new Order($env->client($http->stack()), static function (string $line) use (&$output): void {
            $output .= $line . "\n";
        });
        self::assertSame($exit, $order->run());

        return $output;
    }
}
