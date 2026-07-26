// Dictionary Test - 测试词典查询功能

use moontranslator_lib::services::multi_dictionary::MultiSourceDictionary;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 词典查询功能测试 ===\n");

    let dict = MultiSourceDictionary::new();

    // 测试单词列表
    let test_words = vec!["hello", "brilliant", "computer", "comprehensive", "test"];

    for word in test_words {
        println!("🔍 查询单词: {}", word);
        println!("{}", "-".repeat(60));

        match dict.lookup(word).await {
            Ok(entries) => {
                for entry in entries {
                    println!("📖 单词: {}", entry.word);
                    println!("📍 来源: {}", entry.source);

                    // 音标
                    if !entry.phonetics.is_empty() {
                        for phonetic in &entry.phonetics {
                            if let Some(text) = &phonetic.text {
                                println!("🔊 音标: {}", text);
                            }
                            if let Some(audio) = &phonetic.audio {
                                println!("🎵 发音: {}", audio);
                            }
                        }
                    }

                    // 释义
                    println!("\n📝 释义:");
                    for (i, meaning) in entry.meanings.iter().enumerate() {
                        println!("\n  [{}] {}", i + 1, meaning.part_of_speech);
                        for (j, def) in meaning.definitions.iter().enumerate() {
                            println!("    {}. {}", j + 1, def.definition);
                            if let Some(example) = &def.example {
                                println!("       例: {}", example);
                            }
                            if !def.synonyms.is_empty() {
                                println!("       同义: {}", def.synonyms.join(", "));
                            }
                            if !def.antonyms.is_empty() {
                                println!("       反义: {}", def.antonyms.join(", "));
                            }
                        }
                    }
                }
                println!("\n✅ 查询成功\n");
            },
            Err(e) => {
                println!("❌ 查询失败: {}\n", e);
            },
        }
    }

    println!("=== 测试完成 ===");
    Ok(())
}
