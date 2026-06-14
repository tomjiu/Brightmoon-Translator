// Event Store 使用示例

use anyhow::Result;
use moontranslator_lib::domain::{BaseData, CardEvent, WordCard};
use moontranslator_lib::infrastructure::EventStore;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Event Store 示例 ===\n");

    // 1. 创建 Event Store
    let store = EventStore::new("sqlite::memory:").await?;
    store.init_schema().await?;
    println!("✅ Event Store 初始化完成\n");

    // 2. 创建卡牌 ID
    let card_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    // 3. 事件流：导入单词
    println!("📝 事件 1: 导入单词 'brilliant'");
    let event1 = CardEvent::WordImported {
        word: "brilliant".to_string(),
        source: "manual".to_string(),
        timestamp: now,
    };
    store.append_event(&card_id, &event1).await?;

    // 4. 事件流：AI 生成内容
    println!("📝 事件 2: AI 生成助记法");
    let event2 = CardEvent::AiContentGenerated {
        content: moontranslator_lib::domain::AiContent {
            etymology: None,
            mnemonics: vec![moontranslator_lib::domain::Mnemonic {
                mnemonic_type: moontranslator_lib::domain::MnemonicType::Etymology,
                content: "brill-(闪耀) + -iant(形容词后缀) → 闪耀的 → 出色的".to_string(),
                score: None,
            }],
            examples: vec![],
            scenes: vec![],
        },
        model: "gpt-4".to_string(),
        confidence: 0.9,
        timestamp: now + 1000,
    };
    store.append_event(&card_id, &event2).await?;

    // 5. 事件流：用户打分
    println!("📝 事件 3: 用户给助记法打 5 分");
    let event3 = CardEvent::UserRated {
        field: "mnemonic".to_string(),
        score: 5.0,
        feedback: Some("很好记！".to_string()),
        timestamp: now + 2000,
    };
    store.append_event(&card_id, &event3).await?;

    println!("\n=== 从事件流重建卡牌 ===\n");

    // 6. 从事件流重建卡牌
    let card = store.rebuild_card(&card_id).await?;
    println!("✅ 卡牌重建成功:");
    println!("   单词: {}", card.word);
    println!("   版本: {}", card.current_version);
    println!("   创建时间: {}", card.created_at);
    println!("   更新时间: {}", card.updated_at);

    if let Some(ai_content) = &card.ai_content {
        println!("   助记法数量: {}", ai_content.mnemonics.len());
        if let Some(mnemonic) = ai_content.mnemonics.first() {
            println!("   助记法内容: {}", mnemonic.content);
        }
    }

    // 7. 统计事件
    let event_count = store.count_events(&card_id).await?;
    println!("\n📊 总事件数: {}", event_count);

    // 8. 时间旅行：查看 1 秒前的状态
    println!("\n=== 时间旅行：查看创建后 1 秒的状态 ===\n");
    let card_at_past = store.get_card_at_time(&card_id, now + 1500).await?;
    println!("   那时的版本: {}", card_at_past.current_version);
    println!("   那时的更新时间: {}", card_at_past.updated_at);

    // 9. 更新快照（性能优化）
    println!("\n=== 更新快照 ===");
    store.update_snapshot(&card).await?;
    println!("✅ 快照已保存\n");

    // 10. 从快照加载（快速）
    println!("=== 从快照加载 ===");
    if let Some(snapshot_card) = store.load_snapshot(&card_id).await? {
        println!("✅ 从快照加载成功（快速查询）");
        println!("   单词: {}", snapshot_card.word);
    }

    println!("\n🎉 Event Store 演示完成！");

    Ok(())
}
