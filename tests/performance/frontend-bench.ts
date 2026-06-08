/**
 * Frontend Performance Benchmark Tests
 *
 * This module provides performance testing utilities for the Moon Translator frontend.
 * Run with: npx tsx tests/performance/frontend-bench.ts
 */

interface PerformanceResult {
  name: string;
  duration: number;
  iterations: number;
  avgMs: number;
  minMs: number;
  maxMs: number;
  p95Ms: number;
  p99Ms: number;
}

class PerformanceBenchmark {
  private results: PerformanceResult[] = [];

  async run(
    name: string,
    fn: () => void | Promise<void>,
    iterations: number = 100
  ): Promise<PerformanceResult> {
    const durations: number[] = [];

    // Warmup
    for (let i = 0; i < 10; i++) {
      await fn();
    }

    // Benchmark
    for (let i = 0; i < iterations; i++) {
      const start = performance.now();
      await fn();
      const end = performance.now();
      durations.push(end - start);
    }

    // Calculate statistics
    durations.sort((a, b) => a - b);
    const sum = durations.reduce((a, b) => a + b, 0);
    const avg = sum / durations.length;
    const min = durations[0];
    const max = durations[durations.length - 1];
    const p95Index = Math.floor(durations.length * 0.95);
    const p99Index = Math.floor(durations.length * 0.99);

    const result: PerformanceResult = {
      name,
      duration: sum,
      iterations,
      avgMs: avg,
      minMs: min,
      maxMs: max,
      p95Ms: durations[p95Index],
      p99Ms: durations[p99Index],
    };

    this.results.push(result);
    return result;
  }

  getResults(): PerformanceResult[] {
    return this.results;
  }

  printResults(): void {
    console.log('\n=== Performance Benchmark Results ===\n');

    for (const result of this.results) {
      console.log(`Benchmark: ${result.name}`);
      console.log(`  Iterations: ${result.iterations}`);
      console.log(`  Total Duration: ${result.duration.toFixed(2)}ms`);
      console.log(`  Average: ${result.avgMs.toFixed(3)}ms`);
      console.log(`  Min: ${result.minMs.toFixed(3)}ms`);
      console.log(`  Max: ${result.maxMs.toFixed(3)}ms`);
      console.log(`  P95: ${result.p95Ms.toFixed(3)}ms`);
      console.log(`  P99: ${result.p99Ms.toFixed(3)}ms`);
      console.log('');
    }
  }

  exportMarkdown(): string {
    let md = '# Frontend Performance Benchmark Results\n\n';
    md += `Generated: ${new Date().toISOString()}\n\n`;

    md += '## Summary\n\n';
    md += '| Benchmark | Avg (ms) | P95 (ms) | P99 (ms) | Min (ms) | Max (ms) | Iterations |\n';
    md += '|-----------|----------|----------|----------|----------|----------|------------|\n';

    for (const result of this.results) {
      md += `| ${result.name} | ${result.avgMs.toFixed(3)} | ${result.p95Ms.toFixed(3)} | ${result.p99Ms.toFixed(3)} | ${result.minMs.toFixed(3)} | ${result.maxMs.toFixed(3)} | ${result.iterations} |\n`;
    }

    md += '\n## Detailed Results\n\n';

    for (const result of this.results) {
      md += `### ${result.name}\n\n`;
      md += `- **Iterations**: ${result.iterations}\n`;
      md += `- **Total Duration**: ${result.duration.toFixed(2)}ms\n`;
      md += `- **Average**: ${result.avgMs.toFixed(3)}ms\n`;
      md += `- **Min**: ${result.minMs.toFixed(3)}ms\n`;
      md += `- **Max**: ${result.maxMs.toFixed(3)}ms\n`;
      md += `- **P95**: ${result.p95Ms.toFixed(3)}ms\n`;
      md += `- **P99**: ${result.p99Ms.toFixed(3)}ms\n\n`;
    }

    return md;
  }
}

// String processing benchmarks
function benchmarkStringProcessing(bench: PerformanceBenchmark) {
  const testStrings = {
    short: 'Hello',
    medium: 'Hello, world! This is a test sentence.',
    long: 'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '.repeat(10),
    unicode: '你好世界こんにちは世界',
  };

  // String concatenation
  bench.run('string_concat_short', () => {
    let result = '';
    for (let i = 0; i < 1000; i++) {
      result += testStrings.short;
    }
    return result;
  });

  // Template literal
  bench.run('string_template_short', () => {
    let result = '';
    for (let i = 0; i < 1000; i++) {
      result = `${result}${testStrings.short}`;
    }
    return result;
  });

  // Array join
  bench.run('string_array_join', () => {
    const parts: string[] = [];
    for (let i = 0; i < 1000; i++) {
      parts.push(testStrings.short);
    }
    return parts.join('');
  });

  // Regex operations
  bench.run('regex_match', () => {
    const regex = /hello/gi;
    return testStrings.long.match(regex);
  });

  bench.run('regex_replace', () => {
    return testStrings.long.replace(/lorem/gi, 'REPLACED');
  });

  // String split
  bench.run('string_split', () => {
    return testStrings.long.split(' ');
  });

  // Unicode processing
  bench.run('unicode_length', () => {
    return testStrings.unicode.length;
  });

  bench.run('unicode_iteration', () => {
    let count = 0;
    for (const _char of testStrings.unicode) {
      count++;
    }
    return count;
  });
}

// JSON processing benchmarks
function benchmarkJsonProcessing(bench: PerformanceBenchmark) {
  const smallObj = { name: 'test', value: 42 };
  const mediumObj = {
    items: Array.from({ length: 100 }, (_, i) => ({
      id: i,
      name: `item_${i}`,
      value: Math.random(),
    })),
  };
  const largeObj = {
    data: Array.from({ length: 1000 }, (_, i) => ({
      id: i,
      text: `Translation result ${i}`.repeat(5),
      metadata: { engine: 'google', latency: Math.random() * 100 },
    })),
  };

  // JSON stringify
  bench.run('json_stringify_small', () => {
    return JSON.stringify(smallObj);
  });

  bench.run('json_stringify_medium', () => {
    return JSON.stringify(mediumObj);
  });

  bench.run('json_stringify_large', () => {
    return JSON.stringify(largeObj);
  });

  // JSON parse
  const smallJson = JSON.stringify(smallObj);
  const mediumJson = JSON.stringify(mediumObj);
  const largeJson = JSON.stringify(largeObj);

  bench.run('json_parse_small', () => {
    return JSON.parse(smallJson);
  });

  bench.run('json_parse_medium', () => {
    return JSON.parse(mediumJson);
  });

  bench.run('json_parse_large', () => {
    return JSON.parse(largeJson);
  });
}

// DOM manipulation benchmarks
function benchmarkDomOperations(bench: PerformanceBenchmark) {
  // Create a mock DOM environment
  const mockElement = {
    innerHTML: '',
    style: {} as Record<string, string>,
    classList: {
      add: () => {},
      remove: () => {},
      toggle: () => {},
    },
    appendChild: () => {},
    removeChild: () => {},
    querySelectorAll: () => [],
  };

  // InnerHTML update
  bench.run('dom_innerhtml_update', () => {
    const html = '<div>' + 'x'.repeat(1000) + '</div>';
    mockElement.innerHTML = html;
  });

  // Style update
  bench.run('dom_style_update', () => {
    mockElement.style.color = 'red';
    mockElement.style.fontSize = '14px';
    mockElement.style.display = 'block';
  });

  // ClassList operations
  bench.run('dom_classlist_operations', () => {
    mockElement.classList.add('active');
    mockElement.classList.remove('hidden');
    mockElement.classList.toggle('selected');
  });
}

// Array processing benchmarks
function benchmarkArrayProcessing(bench: PerformanceBenchmark) {
  const smallArray = Array.from({ length: 100 }, (_, i) => i);
  const mediumArray = Array.from({ length: 1000 }, (_, i) => i);
  const largeArray = Array.from({ length: 10000 }, (_, i) => i);

  // Map
  bench.run('array_map_small', () => {
    return smallArray.map((x) => x * 2);
  });

  bench.run('array_map_medium', () => {
    return mediumArray.map((x) => x * 2);
  });

  bench.run('array_map_large', () => {
    return largeArray.map((x) => x * 2);
  });

  // Filter
  bench.run('array_filter_small', () => {
    return smallArray.filter((x) => x % 2 === 0);
  });

  bench.run('array_filter_medium', () => {
    return mediumArray.filter((x) => x % 2 === 0);
  });

  // Reduce
  bench.run('array_reduce_small', () => {
    return smallArray.reduce((acc, x) => acc + x, 0);
  });

  bench.run('array_reduce_medium', () => {
    return mediumArray.reduce((acc, x) => acc + x, 0);
  });

  // Sort
  bench.run('array_sort_small', () => {
    return [...smallArray].sort((a, b) => b - a);
  });

  bench.run('array_sort_medium', () => {
    return [...mediumArray].sort((a, b) => b - a);
  });

  // Find
  bench.run('array_find_small', () => {
    return smallArray.find((x) => x === 50);
  });

  bench.run('array_find_medium', () => {
    return mediumArray.find((x) => x === 500);
  });

  // Includes
  bench.run('array_includes_small', () => {
    return smallArray.includes(50);
  });

  bench.run('array_includes_medium', () => {
    return mediumArray.includes(500);
  });
}

// Translation result rendering simulation
function benchmarkTranslationRendering(bench: PerformanceBenchmark) {
  // Simulate translation result processing
  const translationResults = Array.from({ length: 50 }, (_, i) => ({
    engine: `engine_${i % 5}`,
    text: `Translation result ${i}. `.repeat(10),
    latency: Math.random() * 200,
  }));

  // Format results
  bench.run('format_translation_results', () => {
    return translationResults.map((r) => ({
      ...r,
      formatted: `[${r.engine}] ${r.text}`,
      latencyStr: `${r.latency.toFixed(0)}ms`,
    }));
  });

  // Sort by latency
  bench.run('sort_translations_by_latency', () => {
    return [...translationResults].sort((a, b) => a.latency - b.latency);
  });

  // Filter by engine
  bench.run('filter_translations_by_engine', () => {
    return translationResults.filter((r) => r.engine === 'engine_0');
  });

  // Generate HTML for results
  bench.run('generate_results_html', () => {
    const items = translationResults
      .map(
        (r) => `
        <div class="result-item">
          <span class="engine">${r.engine}</span>
          <p class="text">${r.text}</p>
          <span class="latency">${r.latency.toFixed(0)}ms</span>
        </div>
      `
      )
      .join('');
    return `<div class="results">${items}</div>`;
  });
}

// Large text processing
function benchmarkLargeTextProcessing(bench: PerformanceBenchmark) {
  const largeText = 'Hello world. '.repeat(10000);

  // Split into sentences
  bench.run('split_sentences', () => {
    return largeText.split(/[.!?]+/).filter((s) => s.trim());
  });

  // Word count
  bench.run('word_count', () => {
    return largeText.split(/\s+/).length;
  });

  // Character frequency
  bench.run('char_frequency', () => {
    const freq: Record<string, number> = {};
    for (const char of largeText) {
      freq[char] = (freq[char] || 0) + 1;
    }
    return freq;
  });

  // Text truncation
  bench.run('text_truncation', () => {
    const maxLength = 1000;
    if (largeText.length > maxLength) {
      return largeText.substring(0, maxLength) + '...';
    }
    return largeText;
  });
}

// Main execution
async function main() {
  const bench = new PerformanceBenchmark();

  console.log('Running frontend performance benchmarks...\n');

  // Run all benchmarks
  benchmarkStringProcessing(bench);
  benchmarkJsonProcessing(bench);
  benchmarkDomOperations(bench);
  benchmarkArrayProcessing(bench);
  benchmarkTranslationRendering(bench);
  benchmarkLargeTextProcessing(bench);

  // Print results
  bench.printResults();

  // Export markdown report
  const report = bench.exportMarkdown();

  // In a real environment, you would write this to a file:
  // import { writeFileSync } from 'fs';
  // writeFileSync('docs/PERFORMANCE_FRONTEND.md', report);

  console.log('\n=== Report Preview ===\n');
  console.log(report.substring(0, 2000) + '...\n');

  console.log('Benchmark complete!');
}

// Run if executed directly
if (typeof require !== 'undefined' && require.main === module) {
  main().catch(console.error);
}

export { PerformanceBenchmark, main };
