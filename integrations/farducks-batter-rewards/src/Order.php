<?php

declare(strict_types=1);

namespace Farducks;

use Lingara\Client;
use Lingara\Exception\LingaraException;
use Lingara\Model\LessonPlanCreateRequest;
use Lingara\Stream\CreateLessonPlanEvent\Result;
use Lingara\Stream\CreateLessonPlanEvent\Started;

/**
 * One order: while it fries, the backend creates the customer's lesson plan,
 * takes its id, then reads the plan back by that id alone, which is what a
 * till or a later request does. Prints one line per step.
 */
final class Order
{
    /** @param \Closure(string): void $print */
    public function __construct(
        private readonly Client $client,
        private readonly \Closure $print,
        private readonly string $context = 'ordering fish and chips at the counter',
        private readonly string $sourceLang = 'en',
        private readonly string $targetLang = 'es',
        private readonly int $level = 2,
    ) {
    }

    /** The process exit code. */
    public function run(): int
    {
        try {
            $id = $this->create();
            ($this->print)("order: plan {$id}");
            $plan = $this->client->getLessonPlan($id)->value;
            ($this->print)(sprintf('read back: "%s" (%s)', $plan->getTitle() ?? '(untitled)', $plan->getStatus()->value));

            return 0;
        } catch (LingaraException $e) {
            // The library keeps the secret and every token out of its
            // messages, so the message is safe to print.
            ($this->print)('order failed: ' . $e->getMessage());

            return 1;
        }
    }

    /**
     * The plan's id: a `started` event carries it, and a plan served from
     * Lingara's pool arrives as a lone `result` with no `started`.
     */
    private function create(): string
    {
        $stream = $this->client->createLessonPlan(new LessonPlanCreateRequest([
            'context' => $this->context,
            'source_lang' => $this->sourceLang,
            'target_lang' => $this->targetLang,
            'level' => $this->level,
        ]));
        foreach ($stream as $event) {
            if ($event instanceof Started) {
                return $event->data->getPlanId();
            }
            if ($event instanceof Result) {
                return $event->data->getPlan()->getId();
            }
        }
        throw new \RuntimeException('the stream ended without a plan id');
    }
}
