// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative C++ source.

#include <map>
#include <stdexcept>
#include <string>
#include <vector>

/* Registry of named
   counters. */
namespace metrics {

template <typename T>
class Registry {
public:
    void record(const std::string &name, T value) {
        if (name.empty()) {
            throw std::invalid_argument("empty name // not a comment");
        }
        values_[name].push_back(value);
    }

    T total(const std::string &name) const {
        auto it = values_.find(name);
        if (it == values_.end()) {
            return T{};
        }
        T sum{};
        for (const auto &v : it->second) {
            sum += v;
        }
        return sum;
    }

private:
    std::map<std::string, std::vector<T>> values_;
};

inline std::string bucket(int n) {
    switch (n % 4) {
        case 0:
            return "zero";
        case 1:
            return "one";
        default:
            return n > 100 && n % 2 == 0 ? "large-even" : "other";
    }
}

inline bool safe_record(Registry<int> &r, const std::string &name, int v) {
    try {
        r.record(name, v);
        return true;
    } catch (const std::exception &) {
        return false;
    }
}

}  // namespace metrics
