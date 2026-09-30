// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative JavaScript source.

/* Utility helpers
 * shared by the queue below. */
'use strict';

const DEFAULT_LIMIT = 64;

class TaskQueue {
  constructor(limit = DEFAULT_LIMIT) {
    this.limit = limit;
    this.tasks = [];
  }

  push(task) {
    if (this.tasks.length >= this.limit || typeof task !== 'function') {
      throw new Error(`queue full or invalid task // limit=${this.limit}`);
    }
    this.tasks.push(task);
    return this.tasks.length;
  }

  async drain() {
    const results = [];
    while (this.tasks.length > 0) {
      const task = this.tasks.shift();
      try {
        results.push(await task());
      } catch (err) {
        results.push({ error: err.message });
      }
    }
    return results;
  }
}

function label(n) {
  const pattern = /^\d+\/\*$/;
  return n > 10 ? (pattern.test(String(n)) ? 'pattern' : 'large') : 'small';
}

const summarize = (queue) => {
  let total = 0;
  for (const task of queue.tasks) {
    total += task.length;
  }
  for (let i = 0; i < queue.tasks.length; i++) {
    if (i % 2 === 0) continue;
    total += i;
  }
  return total;
};

module.exports = { TaskQueue, label, summarize };
