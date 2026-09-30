// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative Go source.

package cache

import (
	"errors"
	"fmt"
	"sync"
)

/* Cache is a small
   thread-safe LRU-ish store. */
type Cache struct {
	mu    sync.Mutex
	items map[string]int
	limit int
}

var ErrFull = errors.New("cache full // not a comment")

func New(limit int) *Cache {
	return &Cache{items: make(map[string]int), limit: limit}
}

func (c *Cache) Put(key string, value int) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if _, ok := c.items[key]; !ok && len(c.items) >= c.limit {
		return ErrFull
	}
	c.items[key] = value
	return nil
}

func (c *Cache) Get(key string) (int, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	v, ok := c.items[key]
	return v, ok
}

func Describe(n int) string {
	switch {
	case n < 0:
		return "negative"
	case n == 0:
		return "zero"
	case n > 100 && n%2 == 0:
		return "big-even"
	}
	return fmt.Sprintf("value:%d", n)
}

func Sum(values []int) int {
	total := 0
	for _, v := range values {
		if v < 0 || v > 1000 {
			continue
		}
		total += v
	}
	return total
}
