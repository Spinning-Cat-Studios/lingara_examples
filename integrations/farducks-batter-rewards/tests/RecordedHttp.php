<?php

declare(strict_types=1);

namespace Farducks\Tests;

use Lingara\HttpStack;
use Nyholm\Psr7\Factory\Psr17Factory;
use Nyholm\Psr7\Response;
use Psr\Http\Client\ClientInterface;
use Psr\Http\Message\RequestInterface;
use Psr\Http\Message\ResponseInterface;

/**
 * A PSR-18 client that answers from a queue of recorded responses and keeps
 * every request it was sent, so no request leaves the process.
 */
final class RecordedHttp implements ClientInterface
{
    /** @var list<RequestInterface> */
    public array $sent = [];

    /** @param list<ResponseInterface> $responses */
    public function __construct(private array $responses)
    {
    }

    public static function json(int $status, string $body): ResponseInterface
    {
        return new Response($status, ['Content-Type' => 'application/json'], $body);
    }

    /** @param list<array{string, string}> $events each an event name and its JSON data */
    public static function sse(array $events): ResponseInterface
    {
        $body = '';
        foreach ($events as [$name, $data]) {
            $body .= "event: {$name}\ndata: {$data}\n\n";
        }

        return new Response(200, ['Content-Type' => 'text/event-stream'], $body);
    }

    public function stack(): HttpStack
    {
        $factory = new Psr17Factory();

        return HttpStack::custom($this, $factory, $factory);
    }

    public function sendRequest(RequestInterface $request): ResponseInterface
    {
        $this->sent[] = $request;
        $next = array_shift($this->responses);
        if ($next === null) {
            throw new \LogicException("no recorded response for {$request->getMethod()} {$request->getUri()}");
        }

        return $next;
    }

    /** @return list<string> each request as "METHOD /path" */
    public function routes(): array
    {
        return array_map(static fn (RequestInterface $r): string => "{$r->getMethod()} {$r->getUri()->getPath()}", $this->sent);
    }
}
