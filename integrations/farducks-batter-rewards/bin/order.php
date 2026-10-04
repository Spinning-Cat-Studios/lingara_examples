<?php

declare(strict_types=1);

// One order: create the plan while it fries, then read it back by its id.
//
//   LINGARA_CLIENT_ID=… LINGARA_CLIENT_SECRET=… php bin/order.php

require __DIR__ . '/../vendor/autoload.php';

$client = Farducks\Environment::fromProcess()->client();
exit((new Farducks\Order($client, static function (string $line): void { echo $line, PHP_EOL; }))->run());
