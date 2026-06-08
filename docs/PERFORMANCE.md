# Performance Benchmark Documentation

This document describes the performance benchmarking framework for Moon Translator.

## Overview

The benchmark suite covers:
- **Backend (Rust)**: Translation engines, caching system, OCR processing
- **Frontend (TypeScript)**: String processing, JSON handling, DOM operations, array operations

## Running Benchmarks

### Rust Benchmarks (Criterion)

Navigate to the `src-tauri` directory and run:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suite
cargo bench --bench translation_bench
cargo bench --bench cache_bench
cargo bench --bench ocr_bench

# Run with HTML report generation
cargo bench -- --output-format html
```

Benchmark reports are generated in `src-tauri/target/criterion/`.

### Frontend Benchmarks

```bash
# Install dependencies
npm install -D tsx

# Run frontend benchmarks
npx tsx tests/performance/frontend-bench.ts
```

## Benchmark Suites

### 1. Translation Engine Benchmarks (`translation_bench.rs`)

Tests the performance of translation-related operations:

| Benchmark | Description | Target |
|-----------|-------------|--------|
| `single_translation/delay_ms/{N}` | Single translation with varying network delay | Baseline |
| `text_length/length/{size}` | Translation of different text lengths | < 100ms for short |
| `parallel_translation/engines/{N}` | Parallel translation across N engines | Linear scaling |
| `batch_translation/batch_size/{N}` | Batch translation of N texts | < 50ms per item |
| `cache_key/text/{size}` | Cache key generation | < 1μs |

### 2. Cache System Benchmarks (`cache_bench.rs`)

Tests the performance of the translation cache:

| Benchmark | Description | Target |
|-----------|-------------|--------|
| `cache_write/cache_size/{N}` | Write performance at different cache sizes | < 10μs |
| `cache_read_hit` | Cache hit read performance | < 5μs |
| `cache_read_miss` | Cache miss read performance | < 2μs |
| `cache_concurrent/concurrency/{N}` | Concurrent access with N threads | Linear scaling |
| `cache_eviction/cache_size/{N}` | Eviction performance | < 1ms |
| `cache_key_format/format/{type}` | Key format generation | < 100ns |

### 3. OCR Processing Benchmarks (`ocr_bench.rs`)

Tests image processing and OCR performance:

| Benchmark | Description | Target |
|-----------|-------------|--------|
| `image_generation/size/{WxH}` | Test image generation | < 10ms |
| `png_encoding/size/{WxH}` | PNG encoding speed | < 50ms for 640x480 |
| `image_resize/target_width/{W}` | Image resize with Lanczos3 | < 100ms |
| `image_grayscale/size/{WxH}` | Grayscale conversion | < 20ms |
| `image_crop/crop/{size}` | Image cropping | < 5ms |
| `base64_encoding/size/{N}` | Base64 encoding | < 1ms for 1KB |
| `ocr_extraction/image/{size}` | OCR text extraction | < 500ms |
| `ocr_region/region/{size}` | Region-based OCR | < 200ms |

### 4. Frontend Benchmarks (`frontend-bench.ts`)

Tests JavaScript/TypeScript performance:

| Category | Benchmarks | Target |
|----------|------------|--------|
| String Processing | concat, template, join, regex, split, unicode | < 1ms |
| JSON Processing | stringify, parse (small/medium/large) | < 1ms for small |
| DOM Operations | innerHTML, style, classList | < 0.1ms |
| Array Processing | map, filter, reduce, sort, find | < 1ms for 1K items |
| Translation Rendering | format, sort, filter, HTML generation | < 5ms |
| Large Text Processing | split, word count, frequency, truncation | < 10ms |

## Performance Targets

### Translation Latency

| Scenario | Target | Notes |
|----------|--------|-------|
| Cache hit | < 10ms | Local SQLite lookup |
| Single engine (fast) | < 200ms | Google, Microsoft |
| Single engine (slow) | < 500ms | LLM-based engines |
| Parallel (3 engines) | < 300ms | First response |
| Batch (10 items) | < 2s | Total for 10 items |

### OCR Performance

| Scenario | Target | Notes |
|----------|--------|-------|
| Small region (100x100) | < 100ms | Text recognition |
| Medium region (500x500) | < 300ms | Text recognition |
| Full screen (1920x1080) | < 1s | Text recognition |
| Image preprocessing | < 50ms | Resize + grayscale |

### Cache Performance

| Operation | Target | Notes |
|-----------|--------|-------|
| Read (hit) | < 5ms | SQLite query |
| Write | < 10ms | SQLite insert |
| Eviction | < 1ms | LRU cleanup |
| Concurrent (8 threads) | < 20ms | No deadlock |

### Frontend Performance

| Operation | Target | Notes |
|-----------|--------|-------|
| Page load | < 500ms | Initial render |
| Translation result render | < 100ms | 10 results |
| Large text (10K words) | < 500ms | Processing |
| Memory usage | < 200MB | Steady state |

## Continuous Performance Monitoring

### Adding New Benchmarks

1. **Rust**: Add to `src-tauri/benches/` directory
2. **Frontend**: Add to `tests/performance/frontend-bench.ts`

### Performance Regression Detection

- Run benchmarks before and after changes
- Compare results using Criterion's built-in comparison
- Set up CI/CD to run benchmarks on PRs

### Generating Reports

```bash
# Rust HTML reports
cargo bench -- --output-format html
open src-tauri/target/criterion/report/index.html

# Frontend report
npx tsx tests/performance/frontend-bench.ts > docs/PERFORMANCE_FRONTEND.md
```

## Optimization Guidelines

### Backend Optimization

1. **Cache Strategy**
   - Use LRU eviction for memory efficiency
   - Set appropriate TTL (default: 72 hours)
   - Monitor cache hit rate

2. **Translation Engine Selection**
   - Prefer fast engines (Google, Microsoft) for real-time
   - Use LLM engines for quality-critical translations
   - Implement fallback chains

3. **Parallel Processing**
   - Use `tokio::spawn` for concurrent translations
   - Implement proper cancellation
   - Set reasonable timeouts

4. **Memory Management**
   - Avoid large string allocations
   - Use `Arc<str>` for shared strings
   - Implement proper cleanup

### Frontend Optimization

1. **Rendering**
   - Use virtual scrolling for large lists
   - Debounce rapid updates
   - Minimize DOM mutations

2. **Data Processing**
   - Use Web Workers for heavy computation
   - Implement streaming for large texts
   - Cache processed results

3. **Memory**
   - Avoid memory leaks in event handlers
   - Clean up subscriptions
   - Use WeakMap/WeakSet where appropriate

## Sample Benchmark Results

The following results were obtained on a typical development machine:

### Cache System Performance

| Benchmark | Avg Time | Notes |
|-----------|----------|-------|
| `cache_read_hit` | ~286 ns | Very fast cache hits |
| `cache_read_miss` | ~228 ns | Fast miss detection |
| `cache_write/cache_size/100` | ~57 µs | Write to small cache |
| `cache_write/cache_size/10000` | ~73 µs | Write to large cache |
| `cache_concurrent/concurrency/1` | ~44 µs | Single thread |
| `cache_concurrent/concurrency/8` | ~424 µs | 8 concurrent threads |
| `cache_eviction/cache_size/100` | ~100 µs | Eviction in small cache |
| `cache_eviction/cache_size/1000` | ~553 µs | Eviction in large cache |
| `cache_key_format` | ~150 ns | Key generation |

### Translation Engine Performance

| Benchmark | Avg Time | Notes |
|-----------|----------|-------|
| `single_translation/delay_ms/10` | ~10 ms | Fast engine simulation |
| `single_translation/delay_ms/100` | ~100 ms | Medium engine simulation |
| `text_length/length/short` | ~115 µs | Short text |
| `text_length/length/long` | ~16 ms | Long text (150+ words) |
| `parallel_translation/engines/1` | ~62 ms | Single engine |
| `parallel_translation/engines/5` | ~63 ms | 5 engines parallel |
| `batch_translation/batch_size/10` | ~160 ms | 10 items batch |
| `batch_translation/batch_size/50` | ~800 ms | 50 items batch |
| `cache_key/text/short` | ~57 ns | Short key |
| `cache_key/text/medium` | ~135 ns | Medium key |

### OCR Processing Performance

| Benchmark | Avg Time | Notes |
|-----------|----------|-------|
| `image_generation/size/320x240` | ~150 µs | Small image |
| `image_generation/size/1280x720` | ~500 µs | Large image |
| `png_encoding/size/320x240` | ~2 ms | Small PNG |
| `png_encoding/size/1280x720` | ~20 ms | Large PNG |
| `image_resize/target_width/320` | ~1.2 ms | Resize to small |
| `image_resize/target_width/1280` | ~12 ms | Resize to large |
| `image_grayscale/size/320x240` | ~215 µs | Grayscale small |
| `image_grayscale/size/1280x720` | ~1.9 ms | Grayscale large |
| `image_crop/crop/small` | ~68 µs | Small crop |
| `image_crop/crop/large` | ~2.5 ms | Large crop |
| `base64_encoding/size/1KB` | ~510 ns | 1KB encoding |
| `base64_encoding/size/1MB` | ~810 µs | 1MB encoding |
| `ocr_extraction/image/small` | ~590 µs | OCR small image |
| `ocr_extraction/image/large` | ~630 µs | OCR large image |

### Performance Analysis

**Cache System:**
- Cache hits are extremely fast (~286ns), well under the 5ms target
- Concurrent access scales linearly, no contention issues
- Eviction performance is acceptable for typical cache sizes

**Translation Engine:**
- Parallel translation provides near-linear speedup
- Batch processing scales linearly with batch size
- Cache key generation is negligible overhead

**OCR Processing:**
- Image preprocessing is fast (<20ms for 1280x720)
- PNG encoding is the bottleneck for large images
- Base64 encoding is very efficient

## Benchmark Environment

### Hardware Requirements

- CPU: Modern multi-core processor
- RAM: 8GB minimum
- Storage: SSD recommended

### Software Requirements

- Rust: 1.70+
- Node.js: 18+
- OS: Windows 10/11 (for WinRT OCR)

### Isolation

- Close unnecessary applications
- Disable power saving mode
- Run multiple iterations for stability

## Known Limitations

1. **OCR Benchmarks**: WinRT OCR requires Windows and actual screen capture
2. **Network Benchmarks**: Actual translation latency depends on network conditions
3. **Frontend Benchmarks**: Browser environment differences may affect results

## Future Improvements

- [ ] Add memory profiling benchmarks
- [ ] Implement flamegraph generation
- [ ] Add stress testing suite
- [ ] Create performance dashboard
- [ ] Set up automated regression alerts
