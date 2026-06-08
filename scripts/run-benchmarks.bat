@echo off
REM Moon Translator Performance Benchmark Runner
REM Usage: scripts\run-benchmarks.bat [suite]

setlocal enabledelayedexpansion

set BENCH_DIR=%~dp0..\src-tauri
set SUITE=%1

if "%SUITE%"=="" (
    echo Running all benchmarks...
    echo.

    echo [1/3] Translation Engine Benchmarks
    echo ====================================
    cd /d "%BENCH_DIR%"
    cargo bench --bench translation_bench
    echo.

    echo [2/3] Cache System Benchmarks
    echo ==============================
    cargo bench --bench cache_bench
    echo.

    echo [3/3] OCR Processing Benchmarks
    echo ================================
    cargo bench --bench ocr_bench
    echo.

    echo All benchmarks complete!
    echo Reports saved to: %BENCH_DIR%\target\criterion\
) else (
    echo Running %SUITE% benchmarks...
    cd /d "%BENCH_DIR%"
    cargo bench --bench %SUITE%
    echo.
    echo Benchmark complete!
    echo Reports saved to: %BENCH_DIR%\target\criterion\%SUITE%
)

endlocal
