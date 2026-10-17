#include "mcharness_speed.h"

#include <cstring>
#include <string>

extern "C" uint64_t mh_stream_push(const char* token, char* out, uint64_t out_cap) {
    if (!token || !out || out_cap == 0) return 0;
    const uint64_t len = std::strlen(token);
    const uint64_t n = len < out_cap ? len : out_cap - 1;
    std::memcpy(out, token, n);
    out[n] = '\0';
    return n;
}
