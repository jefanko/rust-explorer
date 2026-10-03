# Rust Explorer Performance & Stress Benchmark Summary

- **Timestamp**: 1791048396
- **Target**: 100,000 Entries Directory Listing & SQLite FTS5 Search
- **Profile**: release

## Results Summary
- **Snapshot Generation (100k entries)**: ~34 ms (Release)
- **Snapshot Creation**: ~45 ms (Release)
- **Natural Sorting (100k entries)**: ~102 ms (Release) [Target < 150 ms]
- **Paged Slice via Cache (50 items)**: ~26 microseconds [Target < 5 ms]
- **Item Token Lookup**: ~183 nanoseconds [Target < 50 µs]
- **FTS5 100k Indexing Throughput**: ~5,400 items/sec (67.9 MiB DB size on disk) [Budget <= 350 MiB]
- **FTS5 Trigram Substring Search (100k set)**: ~10 ms [Target < 50 ms]

All performance budgets and stress criteria met 100%.
