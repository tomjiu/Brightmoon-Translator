// 数据初始化示例程序

use anyhow::Result;
use moontranslator_lib::infrastructure::DataInitializer;
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== MoonTranslator 数据初始化 ===\n");

    // 1. 连接数据库
    println!("🔌 连接数据库...");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:../data/moontranslator.db".to_string());

    let pool = SqlitePool::connect(&database_url).await?;
    println!("   ✅ 已连接: {}\n", database_url);

    // 2. 创建初始化器
    let initializer = DataInitializer::new(pool.clone());

    // 3. 检查是否已初始化
    let stats = initializer.get_stats().await?;
    if stats.core_vocab_count > 0 {
        println!("⚠️  数据库已初始化");
        println!("   核心词库: {} 个词", stats.core_vocab_count);
        println!("   词根数据: {} 个词", stats.morphology_count);
        println!("   词源数据: {} 个词\n", stats.etymology_count);

        print!("是否重新初始化？这将清除现有数据。[y/N] ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("❌ 已取消");
            return Ok(());
        }

        println!();
    }

    // 4. 执行初始化
    initializer.initialize().await?;

    // 5. 显示统计信息
    let stats = initializer.get_stats().await?;
    println!("\n📊 数据统计:");
    println!("   ├─ 核心词库: {} 个词", stats.core_vocab_count);
    println!("   ├─ 词根数据: {} 个词", stats.morphology_count);
    println!("   └─ 词源数据: {} 个词", stats.etymology_count);

    // 6. 验证数据
    println!("\n🔍 验证数据...");

    // 验证核心词库
    let sample = sqlx::query!(
        r#"
        SELECT word, frequency_rank, frq, tag
        FROM core_vocabulary
        ORDER BY frequency_rank
        LIMIT 10
        "#
    )
    .fetch_all(&pool)
    .await?;

    println!("   前10个高频词:");
    for row in sample {
        println!(
            "      {}. {} (frq: {}, tag: {})",
            row.frequency_rank,
            row.word,
            row.frq.unwrap_or(0),
            row.tag.as_deref().unwrap_or("-")
        );
    }

    // 验证词根数据
    let morphology_sample = sqlx::query!(
        r#"
        SELECT word, segmentation
        FROM morphology
        LIMIT 5
        "#
    )
    .fetch_all(&pool)
    .await?;

    println!("\n   词根数据示例:");
    for row in morphology_sample {
        println!("      {} → {}", row.word, row.segmentation);
    }

    println!("\n✅ 数据验证通过！");
    println!("\n💡 提示:");
    println!("   - 数据库路径: {}", database_url);
    println!("   - 核心词库已按词频排序");
    println!("   - 可以开始使用 DictionarySkill 和 MorphologySkill");

    Ok(())
}
