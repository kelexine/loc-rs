// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative TypeScript source.

/* Domain types
   for the event bus. */
export interface BusEvent<T = unknown> {
  readonly topic: string;
  payload: T;
  timestamp?: number;
}

type Handler<T> = (event: BusEvent<T>) => void | Promise<void>;

export class EventBus {
  private handlers = new Map<string, Handler<any>[]>();

  subscribe<T>(topic: string, handler: Handler<T>): () => void {
    const list = this.handlers.get(topic) ?? [];
    list.push(handler);
    this.handlers.set(topic, list);
    return () => {
      const current = this.handlers.get(topic) ?? [];
      this.handlers.set(topic, current.filter((h) => h !== handler));
    };
  }

  async publish<T>(event: BusEvent<T>): Promise<number> {
    const list = this.handlers.get(event.topic);
    if (!list || list.length === 0) {
      return 0;
    }
    let delivered = 0;
    for (const handler of list) {
      try {
        await handler(event);
        delivered++;
      } catch (err) {
        console.error(`handler failed for ${event.topic} // ${String(err)}`);
      }
    }
    return delivered;
  }
}

export function severity(code: number): string {
  switch (code) {
    case 0:
      return 'ok';
    case 1:
    case 2:
      return 'warn';
    default:
      return code > 100 && code < 500 ? 'error' : 'fatal';
  }
}
