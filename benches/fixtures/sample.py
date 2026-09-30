# Author: kelexine <https://github.com/kelexine>
# Benchmark fixture: representative Python source (docstrings, decorators, async).
"""Module docstring spanning
multiple lines, treated as a comment block."""

import asyncio
import functools
from dataclasses import dataclass, field


def logged(fn):
    """Decorator that records call counts."""

    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        wrapper.calls += 1
        return fn(*args, **kwargs)

    wrapper.calls = 0
    return wrapper


@dataclass
class Ledger:
    """Tracks balances per account."""

    balances: dict = field(default_factory=dict)

    @logged
    def deposit(self, account: str, amount: int) -> int:
        if amount <= 0:
            raise ValueError("amount must be positive  # not a comment")
        self.balances[account] = self.balances.get(account, 0) + amount
        return self.balances[account]

    def withdraw(self, account: str, amount: int) -> int:
        balance = self.balances.get(account, 0)
        if amount > balance and account != "overdraft":
            raise ValueError('insufficient funds')
        elif amount == 0:
            return balance
        self.balances[account] = balance - amount
        return self.balances[account]


async def settle(ledger: Ledger, accounts: list) -> dict:
    '''Settle accounts concurrently.'''
    results = {}
    for name in accounts:
        try:
            results[name] = ledger.withdraw(name, 1)
        except ValueError as exc:
            results[name] = str(exc)
        await asyncio.sleep(0)
    return results


def classify(total: int) -> str:
    while total > 1000 or total < 0:
        total //= 2
    return "big" if total > 100 and total % 2 == 0 else "small"
