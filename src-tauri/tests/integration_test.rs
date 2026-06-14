// Integration Tests - 完整流程集成测试

use moontranslator_lib::domain::{
    AiContent, CardEvent, CardState, Etymology, FsrsEngine, LearningPhase, Mnemonic,
    PersonalizedExample, Rating, Root, Scene, StateMachine, WordCard,
};
use moontranslator_lib::infrastructure::EventStore;
use sqlx::SqlitePool;
use uuid::Uuid;

/// 测试完整的学习流程
#[tokio::test]
async fn test_complete_learning_flow() {
    // 1. 创建内存数据库
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // 2. 初始化 Event Store
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS card_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            event_data TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_card_events_card_id ON card_events(card_id);
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let event_store = EventStore::new(pool.clone());

    // 3. 创建新卡牌
    let card_id = Uuid::new_v4().to_string();
    let word = "brilliant";

    let import_event = CardEvent::WordImported {
        word: word.to_string(),
        source: "test".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    event_store
        .append_event(&card_id, &import_event)
        .await
        .unwrap();

    // 4. 验证卡牌创建
    let card = event_store.rebuild_card(&card_id).await.unwrap();
    assert_eq!(card.word, word);
    assert_eq!(card.current_version, 1);

    // 5. 添加 AI 生成的内容
    let ai_content = AiContent {
        etymology: Some(Etymology {
            origin: "Latin brillare 'to shine'".to_string(),
            root_breakdown: vec![Root {
                part: "brill".to_string(),
                meaning: "shine".to_string(),
                examples: vec!["brilliant".to_string()],
            }],
            historical_usage: Some("Used since 17th century".to_string()),
            cognates: vec!["French: brillant".to_string()],
        }),
        mnemonics: vec![Mnemonic {
            mnemonic_type: "etymology".to_string(),
            content: "Brill-iant = shining bright".to_string(),
            score: Some(0.9),
        }],
        examples: vec![PersonalizedExample {
            text: "She gave a brilliant performance.".to_string(),
            context: "academic".to_string(),
            difficulty: "medium".to_string(),
            score: Some(0.8),
            user_feedback: None,
        }],
        scenes: vec![Scene {
            description: "At a concert".to_string(),
            dialogue: "That was a brilliant show!".to_string(),
            vocabulary_usage: "brilliant = excellent".to_string(),
        }],
    };

    let gen_event = CardEvent::AiContentGenerated {
        content: ai_content.clone(),
        model: "test-model".to_string(),
        confidence: 0.9,
        timestamp: chrono::Utc::now().timestamp(),
    };

    event_store
        .append_event(&card_id, &gen_event)
        .await
        .unwrap();

    // 6. 验证 AI 内容
    let card = event_store.rebuild_card(&card_id).await.unwrap();
    assert!(card.ai_content.is_some());
    assert_eq!(card.current_version, 2);

    // 7. 第一次复习（Good）
    let fsrs = FsrsEngine::new();
    let new_state = fsrs
        .schedule_review(&card.fsrs_state, Rating::Good, chrono::Utc::now())
        .unwrap();

    let fsrs_event = CardEvent::FsrsUpdated {
        grade: Rating::Good,
        new_state: new_state.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    event_store
        .append_event(&card_id, &fsrs_event)
        .await
        .unwrap();

    // 8. 验证 FSRS 状态更新
    let card = event_store.rebuild_card(&card_id).await.unwrap();
    assert_eq!(card.fsrs_state.reps, 1);
    assert!(card.fsrs_state.stability > 0.0);

    // 9. 第二次复习（Easy）
    let new_state = fsrs
        .schedule_review(&card.fsrs_state, Rating::Easy, chrono::Utc::now())
        .unwrap();

    let fsrs_event2 = CardEvent::FsrsUpdated {
        grade: Rating::Easy,
        new_state,
        timestamp: chrono::Utc::now().timestamp(),
    };

    event_store
        .append_event(&card_id, &fsrs_event2)
        .await
        .unwrap();

    // 10. 验证最终状态
    let final_card = event_store.rebuild_card(&card_id).await.unwrap();
    assert_eq!(final_card.fsrs_state.reps, 2);
    assert_eq!(final_card.fsrs_state.lapses, 0);
    assert_eq!(final_card.current_version, 4);

    println!("✅ 完整学习流程测试通过");
}

/// 测试 State Machine
#[tokio::test]
async fn test_state_machine_transitions() {
    // 创建测试卡牌
    let card = WordCard {
        id: Uuid::new_v4().to_string(),
        word: "test".to_string(),
        current_version: 1,
        base_data: Default::default(),
        ai_content: None,
        fsrs_state: CardState::new(),
        error_records: vec![],
        annotations: vec![],
        learning_state: None,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    };

    // 初始状态：New
    let mut state = moontranslator_lib::domain::LearningState::from_card(&card);
    assert_eq!(state.phase, LearningPhase::New);

    // 导入事件
    let event = CardEvent::WordImported {
        word: "test".to_string(),
        source: "test".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    let new_state = StateMachine::process_event(&state, &event, &card).unwrap();
    assert_eq!(new_state.phase, LearningPhase::New);

    // AI 生成内容
    let ai_event = CardEvent::AiContentGenerated {
        content: AiContent {
            etymology: None,
            mnemonics: vec![],
            examples: vec![],
            scenes: vec![],
        },
        model: "test".to_string(),
        confidence: 0.9,
        timestamp: chrono::Utc::now().timestamp(),
    };

    state = StateMachine::process_event(&state, &ai_event, &card).unwrap();
    assert_eq!(state.needs_optimization, false);

    println!("✅ State Machine 测试通过");
}

/// 测试错误场景和优化触发
#[tokio::test]
async fn test_optimization_triggers() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS card_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            event_data TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let event_store = EventStore::new(pool);
    let card_id = Uuid::new_v4().to_string();

    // 1. 导入单词
    let import_event = CardEvent::WordImported {
        word: "difficult".to_string(),
        source: "test".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };
    event_store
        .append_event(&card_id, &import_event)
        .await
        .unwrap();

    // 2. 模拟多次失败
    let card = event_store.rebuild_card(&card_id).await.unwrap();
    let fsrs = FsrsEngine::new();

    for _ in 0..3 {
        let new_state = fsrs
            .schedule_review(&card.fsrs_state, Rating::Again, chrono::Utc::now())
            .unwrap();

        let fsrs_event = CardEvent::FsrsUpdated {
            grade: Rating::Again,
            new_state,
            timestamp: chrono::Utc::now().timestamp(),
        };

        event_store
            .append_event(&card_id, &fsrs_event)
            .await
            .unwrap();
    }

    // 3. 验证触发优化
    let final_card = event_store.rebuild_card(&card_id).await.unwrap();
    assert!(final_card.fsrs_state.lapses >= 3);

    let state = moontranslator_lib::domain::LearningState::from_card(&final_card);
    assert!(StateMachine::should_auto_optimize(&state));

    println!("✅ 优化触发测试通过");
}
