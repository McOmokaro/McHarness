#include "mcharness_speed.h"

#include <cmath>

extern "C" double mh_embed_cosine(const float* a, const float* b, uint64_t dim) {
    if (!a || !b || dim == 0) return 0.0;
    double dot = 0.0, na = 0.0, nb = 0.0;
    for (uint64_t i = 0; i < dim; ++i) {
        dot += static_cast<double>(a[i]) * static_cast<double>(b[i]);
        na += static_cast<double>(a[i]) * static_cast<double>(a[i]);
        nb += static_cast<double>(b[i]) * static_cast<double>(b[i]);
    }
    const double denom = std::sqrt(na) * std::sqrt(nb);
    return denom > 0.0 ? dot / denom : 0.0;
}
