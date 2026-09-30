// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative Java source.

package bench.sample;

import java.util.ArrayList;
import java.util.List;

/* Simple order book
   used as benchmark input. */
public class OrderBook {
    public interface Listener {
        void onFill(String id, int quantity);
    }

    private final List<String> ids = new ArrayList<>();
    private final List<Integer> quantities = new ArrayList<>();
    private Listener listener;

    public void setListener(Listener listener) {
        this.listener = listener;
    }

    public int place(String id, int quantity) {
        if (id == null || id.isEmpty() || quantity <= 0) {
            throw new IllegalArgumentException("invalid order // not a comment");
        }
        ids.add(id);
        quantities.add(quantity);
        return ids.size();
    }

    public int fillAll() {
        int filled = 0;
        for (int q : quantities) {
            filled += q;
        }
        for (String id : ids) {
            if (listener != null) {
                listener.onFill(id, filled);
            }
        }
        return filled;
    }

    public static String tier(int quantity) {
        switch (quantity / 100) {
            case 0:
                return "retail";
            case 1:
                return "wholesale";
            default:
                return quantity > 10_000 && quantity % 2 == 0 ? "institutional" : "bulk";
        }
    }

    public boolean tryPlace(String id, int quantity) {
        try {
            place(id, quantity);
            return true;
        } catch (IllegalArgumentException e) {
            return false;
        }
    }
}
