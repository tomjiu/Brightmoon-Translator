use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Simple in-memory LRU cache for benchmarking
struct SimpleCache {
    data: RwLock<HashMap<String, CacheEntry>>,
    max_size: usize,
}

struct CacheEntry {
    value: String,
    timestamp: i64,
    hits: u64,
}

impl SimpleCache {
    fn new(max_size: usize) -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            max_size,
        }
    }

    async fn get(&self, key: &str) -> Option<String> {
        let mut data = self.data.write().await;
        if let Some(entry) = data.get_mut(key) {
            entry.hits += 1;
            Some(entry.value.clone())
        } else {
            None
        }
    }

    async fn set(&self, key: &str, value: &str) {
        let mut data = self.data.write().await;

        // Evict if at capacity
        if data.len() >= self.max_size {
            // Simple eviction: remove oldest
            if let Some(oldest_key) = data
                .iter()
                .min_by_key(|(_, entry)| entry.timestamp)
                .map(|(k, _)| k.clone())
            {
                data.remove(&oldest_key);
            }
        }

        data.insert(
            key.to_string(),
            CacheEntry {
                value: value.to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                hits: 0,
            },
        );
    }

    async fn size(&self) -> usize {
        self.data.read().await.len()
    }
}

// Benchmark cache write performance
fn bench_cache_write(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_write");

    for cache_size in [100, 1000, 10000] {
        let cache = SimpleCache::new(cache_size);

        group.bench_with_input(
            BenchmarkId::new("cache_size", cache_size),
            &cache_size,
            |b, _| {
                let mut counter = 0;
                b.iter(|| {
                    rt.block_on(async {
                        let key = format!("key_{}", counter);
                        let value = format!("value_{}", counter);
                        cache.set(black_box(&key), black_box(&value)).await;
                        counter += 1;
                    })
                });
            },
        );
    }

    group.finish();
}

// Benchmark cache read performance (cache hit)
fn bench_cache_read_hit(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_read_hit");

    // Pre-populate cache
    let cache = SimpleCache::new(10000);
    rt.block_on(async {
        for i in 0..1000 {
            cache
                .set(&format!("key_{}", i), &format!("value_{}", i))
                .await;
        }
    });

    group.bench_function("hit", |b| {
        let mut counter = 0usize;
        b.iter(|| {
            rt.block_on(async {
                let key = format!("key_{}", counter % 1000);
                counter += 1;
                cache.get(black_box(&key)).await
            })
        });
    });

    group.finish();
}

// Benchmark cache read performance (cache miss)
fn bench_cache_read_miss(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_read_miss");

    let cache = SimpleCache::new(1000);

    group.bench_function("miss", |b| {
        let mut counter = 0usize;
        b.iter(|| {
            rt.block_on(async {
                let key = format!("nonexistent_{}", counter);
                counter += 1;
                cache.get(black_box(&key)).await
            })
        });
    });

    group.finish();
}

// Benchmark concurrent cache access
fn bench_cache_concurrent(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_concurrent");

    for concurrency in [1, 2, 4, 8] {
        let cache = Arc::new(SimpleCache::new(10000));

        // Pre-populate
        rt.block_on(async {
            for i in 0..1000 {
                cache
                    .set(&format!("key_{}", i), &format!("value_{}", i))
                    .await;
            }
        });

        group.bench_with_input(
            BenchmarkId::new("concurrency", concurrency),
            &concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();

                        for thread_id in 0..concurrency {
                            let cache = cache.clone();
                            handles.push(tokio::spawn(async move {
                                let mut results = Vec::new();
                                for i in 0..100 {
                                    let key = format!("key_{}", (thread_id * 100 + i) % 1000);
                                    if let Some(value) = cache.get(&key).await {
                                        results.push(value);
                                    }
                                }
                                results
                            }));
                        }

                        let mut all_results = Vec::new();
                        for handle in handles {
                            if let Ok(results) = handle.await {
                                all_results.extend(results);
                            }
                        }
                        all_results
                    })
                });
            },
        );
    }

    group.finish();
}

// Benchmark cache eviction performance
fn bench_cache_eviction(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_eviction");

    for cache_size in [100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("cache_size", cache_size),
            &cache_size,
            |b, &cache_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let cache = SimpleCache::new(cache_size);
                        // Fill cache to capacity
                        for i in 0..cache_size {
                            cache
                                .set(&format!("key_{}", i), &format!("value_{}", i))
                                .await;
                        }
                        // Trigger evictions
                        for i in 0..100 {
                            cache
                                .set(
                                    &format!("new_key_{}", i),
                                    &format!("new_value_{}", i),
                                )
                                .await;
                        }
                        cache.size().await
                    })
                });
            },
        );
    }

    group.finish();
}

// Benchmark translation cache key format
fn bench_cache_key_format(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_format");

    let test_cases = vec![
        ("short", "en", "zh", "Hello"),
        ("medium", "en", "zh", "Hello, world!"),
        ("long", "en", "zh", "A very long text for benchmarking"),
        ("japanese", "ja", "en", "こんにちは世界"),
        ("chinese", "zh", "en", "你好世界"),
    ];

    for (name, from, to, text) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("format", name),
            &(from, to, text),
            |b, &(from, to, text)| {
                b.iter(|| {
                    format!("{}|{}|{}", from, to, black_box(text))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_cache_write,
    bench_cache_read_hit,
    bench_cache_read_miss,
    bench_cache_concurrent,
    bench_cache_eviction,
    bench_cache_key_format,
);
criterion_main!(benches);
