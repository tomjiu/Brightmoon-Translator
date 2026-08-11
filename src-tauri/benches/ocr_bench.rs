use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use image::{ImageBuffer, Rgb, RgbImage};
use std::io::Cursor;

// Generate a test image for OCR benchmarking
fn generate_test_image(width: u32, height: u32, text: &str) -> Vec<u8> {
    let mut img: RgbImage = ImageBuffer::new(width, height);

    // Fill with white background
    for pixel in img.pixels_mut() {
        *pixel = Rgb([255, 255, 255]);
    }

    // Simple text simulation (draw black pixels in a pattern)
    let text_len = text.len() as u32;
    let char_width = 10;
    let start_x = (width.saturating_sub(text_len * char_width)) / 2;
    let start_y = height / 2 - 5;

    for (i, _ch) in text.chars().enumerate() {
        let x = start_x + (i as u32) * char_width;
        if x + char_width > width {
            break;
        }
        // Draw a simple rectangle for each character
        for dx in 0..char_width.min(8) {
            for dy in 0..12 {
                if x + dx < width && start_y + dy < height {
                    img.put_pixel(x + dx, start_y + dy, Rgb([0, 0, 0]));
                }
            }
        }
    }

    // Encode to PNG
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("Failed to encode image");
    buf.into_inner()
}

// Benchmark image generation
fn bench_image_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_generation");

    for size in [(100, 50), (320, 240), (640, 480), (1280, 720)] {
        let (width, height) = size;
        group.bench_with_input(
            BenchmarkId::new("size", format!("{width}x{height}")),
            &size,
            |b, &(width, height)| {
                b.iter(|| {
                    generate_test_image(
                        black_box(width),
                        black_box(height),
                        black_box("Test OCR Text"),
                    )
                });
            },
        );
    }

    group.finish();
}

// Benchmark PNG encoding
fn bench_png_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("png_encoding");

    for size in [(100, 50), (320, 240), (640, 480), (1280, 720)] {
        let (width, height) = size;
        let img: RgbImage = ImageBuffer::new(width, height);

        group.bench_with_input(
            BenchmarkId::new("size", format!("{width}x{height}")),
            &img,
            |b, img| {
                b.iter(|| {
                    let mut buf = Cursor::new(Vec::new());
                    img.write_to(&mut buf, image::ImageFormat::Png)
                        .expect("Failed to encode");
                    buf.into_inner()
                });
            },
        );
    }

    group.finish();
}

// Benchmark image preprocessing (resize)
fn bench_image_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_resize");

    let img: RgbImage = ImageBuffer::new(1920, 1080);

    for target_width in [320, 640, 1280] {
        group.bench_with_input(
            BenchmarkId::new("target_width", target_width),
            &target_width,
            |b, &target_width| {
                b.iter(|| {
                    let ratio = target_width as f32 / img.width() as f32;
                    let target_height = (img.height() as f32 * ratio) as u32;
                    image::imageops::resize(
                        black_box(&img),
                        target_width,
                        target_height,
                        image::imageops::FilterType::Lanczos3,
                    )
                });
            },
        );
    }

    group.finish();
}

// Benchmark image grayscale conversion
fn bench_image_grayscale(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_grayscale");

    for size in [(320, 240), (640, 480), (1280, 720)] {
        let (width, height) = size;
        let img: RgbImage = ImageBuffer::new(width, height);

        group.bench_with_input(
            BenchmarkId::new("size", format!("{width}x{height}")),
            &img,
            |b, img| {
                b.iter(|| image::imageops::grayscale(black_box(img)));
            },
        );
    }

    group.finish();
}

// Benchmark image cropping
fn bench_image_crop(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_crop");

    let img: RgbImage = ImageBuffer::new(1920, 1080);

    let crops = vec![
        ("small", 100, 100, 200, 200),
        ("medium", 100, 100, 500, 500),
        ("large", 100, 100, 1000, 1000),
    ];

    for (name, x, y, w, h) in crops {
        group.bench_with_input(
            BenchmarkId::new("crop", name),
            &(x, y, w, h),
            |b, &(x, y, w, h)| {
                b.iter(|| image::imageops::crop_imm(black_box(&img), x, y, w, h).to_image());
            },
        );
    }

    group.finish();
}

// Benchmark base64 encoding
fn bench_base64_encoding(c: &mut Criterion) {
    use base64::Engine;

    let mut group = c.benchmark_group("base64_encoding");

    let data_sizes = vec![
        ("1KB", 1024),
        ("10KB", 10240),
        ("100KB", 102400),
        ("1MB", 1048576),
    ];

    for (name, size) in data_sizes {
        let data = vec![0u8; size];

        group.bench_with_input(BenchmarkId::new("size", name), &data, |b, data| {
            b.iter(|| base64::engine::general_purpose::STANDARD.encode(black_box(data)));
        });
    }

    group.finish();
}

// Simulate OCR text extraction (mock)
fn simulate_ocr_extraction(image_data: &[u8], region: Option<(u32, u32, u32, u32)>) -> String {
    // Simulate processing time based on image size
    let size = image_data.len();
    let processing_time = std::time::Duration::from_micros(size as u64 / 100);
    std::thread::sleep(processing_time);

    // Return mock OCR result
    if let Some((x, y, w, h)) = region {
        format!("OCR result for region ({x}, {y}, {w}, {h})")
    } else {
        "OCR result for full image".to_string()
    }
}

// Benchmark OCR extraction simulation
fn bench_ocr_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("ocr_extraction");

    let images = vec![
        ("small", generate_test_image(320, 240, "Test")),
        ("medium", generate_test_image(640, 480, "Test OCR")),
        ("large", generate_test_image(1280, 720, "Test OCR Text")),
    ];

    for (name, image_data) in images {
        group.bench_with_input(
            BenchmarkId::new("image", name),
            &image_data,
            |b, image_data| {
                b.iter(|| simulate_ocr_extraction(black_box(image_data), None));
            },
        );
    }

    group.finish();
}

// Benchmark OCR with region extraction
fn bench_ocr_region_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("ocr_region");

    let image_data = generate_test_image(1920, 1080, "Full screen OCR test");

    let regions = vec![
        ("small", 100, 100, 200, 200),
        ("medium", 100, 100, 500, 500),
        ("large", 100, 100, 1000, 1000),
    ];

    for (name, x, y, w, h) in regions {
        group.bench_with_input(
            BenchmarkId::new("region", name),
            &(x, y, w, h),
            |b, &(x, y, w, h)| {
                b.iter(|| simulate_ocr_extraction(black_box(&image_data), Some((x, y, w, h))));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_image_generation,
    bench_png_encoding,
    bench_image_resize,
    bench_image_grayscale,
    bench_image_crop,
    bench_base64_encoding,
    bench_ocr_extraction,
    bench_ocr_region_extraction,
);
criterion_main!(benches);
