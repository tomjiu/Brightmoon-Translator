#![allow(clippy::unwrap_used, clippy::expect_used)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// Mock translation engine for benchmarking
struct MockTranslationEngine {
    name: String,
    delay_ms: u64,
}

impl MockTranslationEngine {
    fn new(name: &str, delay_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            delay_ms,
        }
    }

    async fn translate(&self, text: &str, _from: &str, _to: &str) -> anyhow::Result<String> {
        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
        Ok(format!("[{}] {}", self.name, text))
    }
}

// Benchmark single translation latency
fn bench_single_translation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("single_translation");

    // Test different engine speeds
    for delay_ms in [10, 50, 100, 200] {
        let engine = MockTranslationEngine::new("mock", delay_ms);
        let text = "Hello, world! This is a test sentence for translation.";

        group.bench_with_input(BenchmarkId::new("delay_ms", delay_ms), &delay_ms, |b, _| {
            b.iter(|| rt.block_on(async { engine.translate(black_box(text), "en", "zh").await }));
        });
    }

    group.finish();
}

// Benchmark translation with different text lengths
fn bench_text_length(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = MockTranslationEngine::new("mock", 10);

    let mut group = c.benchmark_group("text_length");

    let texts = vec![
        ("short", "Hello"),
        ("medium", "Hello, world! This is a medium length sentence for testing translation performance."),
        ("long", "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur."),
    ];

    for (name, text) in texts {
        group.bench_with_input(BenchmarkId::new("length", name), &text, |b, text| {
            b.iter(|| rt.block_on(async { engine.translate(black_box(text), "en", "zh").await }));
        });
    }

    group.finish();
}

// Benchmark parallel translations (simulating multiple engines)
fn bench_parallel_translation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_translation");

    for engine_count in [1, 2, 3, 5] {
        let engines: Vec<MockTranslationEngine> = (0..engine_count)
            .map(|i| MockTranslationEngine::new(&format!("engine_{i}"), 50))
            .collect();

        let text = "Hello, world!";

        group.bench_with_input(
            BenchmarkId::new("engines", engine_count),
            &engines,
            |b, engines| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();
                        for engine in engines {
                            let text = text.to_string();
                            let engine = MockTranslationEngine::new(&engine.name, 50);
                            handles.push(tokio::spawn(async move {
                                engine.translate(&text, "en", "zh").await
                            }));
                        }
                        let mut results = Vec::new();
                        for handle in handles {
                            if let Ok(Ok(result)) = handle.await {
                                results.push(result);
                            }
                        }
                        results
                    })
                });
            },
        );
    }

    group.finish();
}

// Benchmark batch translation performance
fn bench_batch_translation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = MockTranslationEngine::new("mock", 5);

    let mut group = c.benchmark_group("batch_translation");

    for batch_size in [1, 5, 10, 20, 50] {
        let texts: Vec<String> = (0..batch_size)
            .map(|i| format!("Test sentence number {i}"))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            &texts,
            |b, texts| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut results = Vec::new();
                        for text in texts {
                            let result = engine.translate(black_box(text), "en", "zh").await;
                            if let Ok(translated) = result {
                                results.push(translated);
                            }
                        }
                        results
                    })
                });
            },
        );
    }

    group.finish();
}

// Benchmark cache key generation
fn bench_cache_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key");

    let texts = vec![
        ("short", "Hello"),
        ("medium", "A medium length text for cache key generation"),
        ("long", "A very long text that might be used for cache key generation benchmarking purposes in the translation application"),
    ];

    for (name, text) in texts {
        group.bench_with_input(BenchmarkId::new("text", name), &text, |b, text| {
            b.iter(|| {
                // Simulate cache key generation
                let key = format!("{}|{}|{}", "en", "zh", black_box(text));
                key
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_translation,
    bench_text_length,
    bench_parallel_translation,
    bench_batch_translation,
    bench_cache_key_generation,
);
criterion_main!(benches);
