#pragma once
// C ABI surface consumed by the Rust core. Flat types only.

#include <cstdint>

extern "C" {

// Token streaming: push a token into the stream buffer; returns chars flushed.
uint64_t mh_stream_push(const char* token, char* out, uint64_t out_cap);

// Memory search: score a query against memory entries; returns best index, -1 if none.
int64_t mh_memsearch_score(const char* query, const char* const* entries, uint64_t count);

// Embedding math: cosine similarity between two float vectors.
double mh_embed_cosine(const float* a, const float* b, uint64_t dim);

}
