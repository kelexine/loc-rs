<?php
// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative PHP source.

declare(strict_types=1);

namespace Bench\Sample;

/* Minimal session store
   used as benchmark input. */
final class SessionStore
{
    private array $sessions = [];

    # Create or refresh a session, returning the token.
    public function touch(string $user, int $ttl = 3600): string
    {
        if ($user === '' || $ttl <= 0) {
            throw new \InvalidArgumentException("invalid session // not a comment");
        }
        $token = bin2hex(random_bytes(8));
        $this->sessions[$token] = ['user' => $user, 'ttl' => $ttl];
        return $token;
    }

    public function expire(int $maxTtl): int
    {
        $removed = 0;
        foreach ($this->sessions as $token => $session) {
            if ($session['ttl'] > $maxTtl && $session['user'] !== 'admin') {
                unset($this->sessions[$token]);
                $removed++;
            }
        }
        return $removed;
    }
}

function describe(int $n): string
{
    if ($n < 0) {
        return 'negative';
    } elseif ($n === 0) {
        return 'zero';
    } elseif ($n > 100 || $n % 2 === 0) {
        return 'large-or-even';
    }
    return 'odd';
}

function total(array $values): int
{
    $sum = 0;
    for ($i = 0; $i < count($values); $i++) {
        $sum += $values[$i] ?? 0;
    }
    return $sum;
}
?>
