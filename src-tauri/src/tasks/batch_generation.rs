// AI Batch Generation Task - AI 批量预生成后台任务

use crate::domain::{AiContent, CardEvent};
use crate::infrastructure::event_store::EventStore;
use crate::skills::{GenerateCardSkill, OpenAiCompatibleProvider, SkillInput, SkillRegistry};
use anyhow::Result;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

/// 批量生成进度事件
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchGenerationProgress {
    pub task_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub current_word: Option<String>,
    pub status: GenerationStatus,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GenerationStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// 批量生成任务
pub struct BatchGenerationTask {
    task_id: String,
    words: Vec<String>,
    api_key: String,
    base_url: String,
    model: String,
    event_store: Arc<EventStore>,
    app_handle: AppHandle,
    max_concurrent: usize,
}

impl BatchGenerationTask {
    pub fn new(
        task_id: String,
        words: Vec<String>,
        api_key: String,
        base_url: String,
        model: String,
        event_store: Arc<EventStore>,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            task_id,
            words,
            api_key,
            base_url,
            model,
            event_store,
            app_handle,
            max_concurrent: 3, // 并发限制：同时最多3个请求
        }
    }

    /// 运行批量生成任务
    pub async fn run(self) -> Result<()> {
        let total = self.words.len();
        let task_id = self.task_id.clone();
        let app_handle = self.app_handle.clone();

        // 发送开始事件
        self.emit_progress(BatchGenerationProgress {
            task_id: task_id.clone(),
            total,
            completed: 0,
            failed: 0,
            current_word: None,
            status: GenerationStatus::Starting,
        });

        // 创建并发控制信号量
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let mut tasks = vec![];

        let provider = Arc::new(OpenAiCompatibleProvider::new(
            self.api_key.clone(),
            self.base_url.clone(),
            self.model.clone(),
        ));

        let completed = Arc::new(tokio::sync::Mutex::new(0usize));
        let failed = Arc::new(tokio::sync::Mutex::new(0usize));

        // 发送运行中状态
        self.emit_progress(BatchGenerationProgress {
            task_id: task_id.clone(),
            total,
            completed: 0,
            failed: 0,
            current_word: self.words.first().cloned(),
            status: GenerationStatus::Running,
        });

        for word in self.words.clone() {
            let permit = semaphore.clone().acquire_owned().await?;
            let provider_clone = provider.clone();
            let event_store_clone = self.event_store.clone();
            let app_handle_clone = app_handle.clone();
            let task_id_clone = task_id.clone();
            let completed_clone = completed.clone();
            let failed_clone = failed.clone();
            let total_clone = total;

            let task = tokio::spawn(async move {
                let result =
                    generate_word_content(&word, provider_clone, event_store_clone.clone()).await;

                // 更新计数
                match result {
                    Ok(()) => {
                        let mut c = completed_clone.lock().await;
                        *c += 1;
                        let completed_count = *c;
                        drop(c);

                        // 发送进度更新
                        let failed_count = *failed_clone.lock().await;
                        let _ = app_handle_clone.emit(
                            "ai-generation-progress",
                            BatchGenerationProgress {
                                task_id: task_id_clone.clone(),
                                total: total_clone,
                                completed: completed_count,
                                failed: failed_count,
                                current_word: Some(word.clone()),
                                status: GenerationStatus::Running,
                            },
                        );

                        tracing::info!(
                            "✅ AI内容生成成功: {} ({}/{})",
                            word,
                            completed_count,
                            total_clone
                        );
                    },
                    Err(e) => {
                        let mut f = failed_clone.lock().await;
                        *f += 1;
                        let failed_count = *f;
                        drop(f);

                        let completed_count = *completed_clone.lock().await;
                        let _ = app_handle_clone.emit(
                            "ai-generation-progress",
                            BatchGenerationProgress {
                                task_id: task_id_clone.clone(),
                                total: total_clone,
                                completed: completed_count,
                                failed: failed_count,
                                current_word: Some(word.clone()),
                                status: GenerationStatus::Running,
                            },
                        );

                        tracing::warn!("❌ AI内容生成失败: {} - {}", word, e);
                    },
                }

                drop(permit);
            });

            tasks.push(task);
        }

        // 等待所有任务完成
        for task in tasks {
            let _ = task.await;
        }

        let final_completed = *completed.lock().await;
        let final_failed = *failed.lock().await;

        // 发送完成事件
        self.emit_progress(BatchGenerationProgress {
            task_id: task_id.clone(),
            total,
            completed: final_completed,
            failed: final_failed,
            current_word: None,
            status: GenerationStatus::Completed,
        });

        tracing::info!(
            "🎉 批量生成任务完成: {}, 成功: {}, 失败: {}",
            task_id,
            final_completed,
            final_failed
        );

        Ok(())
    }

    fn emit_progress(&self, progress: BatchGenerationProgress) {
        let _ = self.app_handle.emit("ai-generation-progress", progress);
    }
}

/// 为单个单词生成AI内容
async fn generate_word_content(
    word: &str,
    provider: Arc<OpenAiCompatibleProvider>,
    event_store: Arc<EventStore>,
) -> Result<()> {
    // 查找该单词的卡牌
    let pool = event_store.pool();
    let card_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM cards WHERE word = ?1 LIMIT 1")
            .bind(word)
            .fetch_optional(pool)
            .await?;

    let card_id = if let Some(id) = card_id { id } else {
        // 如果卡牌不存在，创建一个
        let new_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let event = CardEvent::WordImported {
            word: word.to_string(),
            source: "batch_generation".to_string(),
            timestamp: now,
        };
        event_store.append_event(&new_id, &event).await?;
        new_id
    };

    // 检查是否已有AI内容
    let existing_content: Option<String> =
        sqlx::query_scalar("SELECT ai_content FROM cards WHERE id = ?1")
            .bind(&card_id)
            .fetch_optional(pool)
            .await?;

    if let Some(content) = existing_content {
        if !content.is_empty() && content != "null" {
            // 已有AI内容，跳过
            return Ok(());
        }
    }

    // 创建技能注册表
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(GenerateCardSkill::new(provider)), 100)?;

    // 准备上下文
    let context = serde_json::json!({
        "word": word,
    });

    let input = SkillInput::new(word).with_param("context", context);

    // 执行生成
    let output = registry.execute("generate_card", input).await?;

    // 解析AI内容
    let ai_content: AiContent = output.into_type()?;

    // 保存事件
    let now = chrono::Utc::now().timestamp();
    let event = CardEvent::AiContentGenerated {
        content: ai_content,
        model: "batch".to_string(),
        confidence: 0.9,
        timestamp: now,
    };

    event_store.append_event(&card_id, &event).await?;

    // 更新快照
    if let Ok(card) = event_store.rebuild_card(&card_id).await {
        event_store.update_snapshot(&card).await?;
    }

    Ok(())
}
