#include "mcharness_speed.h"

#include <cstring>

extern "C" int64_t mh_memsearch_score(const char* query, const char* const* entries, uint64_t count) {
    if (!query || !entries) return -1;
    const size_t qlen = std::strlen(query);
    int64_t best = -1;
    size_t best_score = 0;
    for (uint64_t i = 0; i < count; ++i) {
        if (!entries[i]) continue;
        const size_t elen = std::strlen(entries[i]);
        size_t score = 0;
        const size_t limit = qlen < elen ? qlen : elen;
        for (size_t j = 0; j < limit; ++j) {
            if (query[j] == entries[i][j]) ++score;
        }
        if (score > best_score) {
            best_score = score;
            best = static_cast<int64_t>(i);
        }
    }
    return best;
}
