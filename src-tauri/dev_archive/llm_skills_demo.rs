// LLM Skills 使用示例 - 完整的卡牌生成流程

use anyhow::Result;
use moontranslator_lib::domain::{BaseData, CardEvent, WordCard};
use moontranslator_lib::infrastructure::EventStore;
use moontranslator_lib::skills::{
    DictionarySkill, GenerateCardSkill, LlmProvider, MorphologySkill, OpenAiCompatibleProvider,
    Skill, SkillInput, SkillRegistry,
};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== LLM Skills 完整流程演示 ===\n");

    // 1. 初始化 Event Store
    println!("1️⃣ 初始化 Event Store");
    let event_store = EventStore::new("sqlite::memory:").await?;
    event_store.init_schema().await?;
    println!("   ✅ Event Store 就绪\n");

    // 2. 初始化 Skills
    println!("2️⃣ 初始化 Skills");

    // 注意：这里使用环境变量或配置文件获取 API Key
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        println!("   ⚠️  未设置 OPENAI_API_KEY，使用模拟模式");
        "mock".to_string()
    });

    let llm_provider: Arc<dyn LlmProvider> = Arc::new(OpenAiCompatibleProvider::openai(
        api_key,
        "gpt-4o-mini".to_string(),
    ));

    let mut registry = SkillRegistry::new();

    // 注册词典技能（假设有数据库）
    // let dict_pool = SqlitePool::connect("sqlite:../dictionaries/ecdict.db").await?;
    // registry.register(Box::new(DictionarySkill::new(dict_pool)), 100)?;

    // 注册词根技能（假设有数据）
    let morphology_data = HashMap::new();
    registry.register(Box::new(MorphologySkill::new(morphology_data)), 90)?;

    // 注册 AI 生成技能
    registry.register(Box::new(GenerateCardSkill::new(llm_provider.clone())), 80)?;

    println!("   ✅ 注册了 {} 个技能", registry.list().len());
    for skill in registry.list() {
        println!("      - {} (优先级: {})", skill.name, skill.priority);
    }
    println!();

    // 3. 创建卡牌
    println!("3️⃣ 创建新卡牌");
    let card_id = uuid::Uuid::new_v4().to_string();
    let word = "brilliant";

    let event1 = CardEvent::WordImported {
        word: word.to_string(),
        source: "manual".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    event_store.append_event(&card_id, &event1).await?;
    println!("   ✅ 单词 '{}' 已导入\n", word);

    // 4. 查询词典（如果可用）
    println!("4️⃣ 查询词典");
    // if let Ok(output) = registry
    //     .execute("dictionary", SkillInput::new(word))
    //     .await
    // {
    //     println!("   ✅ 词典查询成功");
    //     if let Some(found) = output.metadata.get("found") {
    //         if found.as_bool().unwrap_or(false) {
    //             println!("   📖 找到释义");
    //         }
    //     }
    // }
    println!("   ⚠️  词典技能未启用（需要数据库）\n");

    // 5. 查询词根（如果可用）
    println!("5️⃣ 查询词根");
    match registry.execute("morphology", SkillInput::new(word)).await {
        Ok(output) => {
            if output.metadata.get("found").and_then(|v| v.as_bool()) == Some(true) {
                println!("   ✅ 找到词根拆解");
            } else {
                println!("   ⚠️  未找到词根数据");
            }
        },
        Err(_) => {
            println!("   ⚠️  词根查询失败");
        },
    }
    println!();

    // 6. AI 生成卡牌内容
    println!("6️⃣ AI 生成学习内容");

    if !llm_provider.is_available() {
        println!("   ⚠️  LLM Provider 不可用，跳过生成");
        println!("\n=== 演示完成 ===");
        return Ok(());
    }

    let context = serde_json::json!({
        "word": word,
        "definition": "extremely intelligent or talented",
        "translation": "出色的，才华横溢的"
    });

    let input = SkillInput::new(word).with_param("context", context);

    println!("   🤖 正在调用 LLM...");
    match registry.execute("generate_card", input).await {
        Ok(output) => {
            println!("   ✅ AI 生成成功");

            // 解析生成的内容
            let ai_content: moontranslator_lib::domain::AiContent =
                serde_json::from_value(output.data)?;

            println!("\n   📝 生成内容预览:");
            println!("      助记法: {} 个", ai_content.mnemonics.len());
            for (i, mnemonic) in ai_content.mnemonics.iter().enumerate() {
                println!(
                    "        {}. [{:?}] {}",
                    i + 1,
                    mnemonic.mnemonic_type,
                    mnemonic.content
                );
            }

            println!("      例句: {} 个", ai_content.examples.len());
            for (i, example) in ai_content.examples.iter().enumerate() {
                println!("        {}. {}", i + 1, example.text);
            }

            if let Some(etym) = &ai_content.etymology {
                println!("      词源: {}", etym.origin);
            }

            // 记录事件
            let event2 = CardEvent::AiContentGenerated {
                content: ai_content,
                model: output
                    .metadata
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                confidence: 0.9,
                timestamp: chrono::Utc::now().timestamp(),
            };

            event_store.append_event(&card_id, &event2).await?;
            println!("\n   ✅ 事件已记录");
        },
        Err(e) => {
            println!("   ❌ AI 生成失败: {}", e);
        },
    }

    println!();

    // 7. 从事件流重建卡牌
    println!("7️⃣ 从事件流重建卡牌");
    let card = event_store.rebuild_card(&card_id).await?;
    println!("   ✅ 卡牌重建成功");
    println!("      单词: ", card.word);
    println!("      版本: {}", card.current_version);
    println!(
        "      事件数: {}",
        event_store.count_events(&card_id).await?
    );
    println!();

    // 8. 更新快照
    println!("8️⃣ 更新快照");
    event_store.update_snapshot(&card).await?;
    println!("   ✅ 快照已保存\n");

    println!("=== 演示完成 ===");
    println!("\n💡 提示:");
    println!("   - 设置 OPENAI_API_KEY 环境变量以启用真实 AI 生成");
    println!("   - 配置词典数据库以启用词典查询");
    println!("   - 加载 MorphoLex 数据以启用词根分析");

    Ok(())
}
