use crate::engine::llm::LlmEngine;
use crate::engine::TranslationEngine;
use crate::security;
use serde::{Deserialize, Serialize};

/// AI polish style options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolishStyle {
    /// Natural and fluent
    Natural,
    /// Formal and professional
    Formal,
    /// Casual and conversational
    Casual,
    /// Technical and precise
    Technical,
    /// Literary and elegant
    Literary,
}

impl PolishStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolishStyle::Natural => "natural",
            PolishStyle::Formal => "formal",
            PolishStyle::Casual => "casual",
            PolishStyle::Technical => "technical",
            PolishStyle::Literary => "literary",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PolishStyle::Natural => "自然流畅",
            PolishStyle::Formal => "正式专业",
            PolishStyle::Casual => "轻松口语",
            PolishStyle::Technical => "技术精确",
            PolishStyle::Literary => "文学优雅",
        }
    }
}

/// Term entry for AI-extracted glossary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTermEntry {
    pub source: String,
    pub target: String,
    pub context: Option<String>,
    pub frequency: u32,
    pub confidence: f32,
}

/// Translation style profile learned from user history
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationStyle {
    pub vocabulary_level: String,
    pub sentence_structure: String,
    pub tone: String,
    pub formality: String,
    pub examples: Vec<StyleExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleExample {
    pub source: String,
    pub translation: String,
}

/// Multi-round translation result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiRoundResult {
    pub rounds: Vec<TranslationRound>,
    pub best_index: usize,
    pub best_translation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRound {
    pub index: usize,
    pub translation: String,
    pub quality_score: f32,
}

/// AI Enhanced translation service
pub struct AiEnhancedService {
    llm_engine: LlmEngine,
}

impl AiEnhancedService {
    pub fn new(llm_engine: LlmEngine) -> Self {
        Self { llm_engine }
    }

    /// Polish translation with specified style
    pub async fn polish_translation(
        &self,
        source_text: &str,
        translated_text: &str,
        from_lang: &str,
        to_lang: &str,
        style: &PolishStyle,
    ) -> Result<String, String> {
        security::validate_text_length(source_text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
        security::validate_text_length(translated_text, security::MAX_TRANSLATION_TEXT_LENGTH)?;

        let lang_name = |code: &str| -> String {
            match code {
                "zh" => "中文".to_string(),
                "en" => "English".to_string(),
                "ja" => "日本語".to_string(),
                "ko" => "한국어".to_string(),
                "fr" => "Français".to_string(),
                "de" => "Deutsch".to_string(),
                "es" => "Español".to_string(),
                "ru" => "Русский".to_string(),
                _ => code.to_string(),
            }
        };

        let style_instruction = match style {
            PolishStyle::Natural => "使译文更加自然流畅，符合日常表达习惯",
            PolishStyle::Formal => "使用正式、专业的措辞，适合商务或学术场合",
            PolishStyle::Casual => "使用轻松、口语化的表达，适合日常对话",
            PolishStyle::Technical => "使用精确的技术术语，保持专业性",
            PolishStyle::Literary => "使用优美的文学语言，注重修辞和韵律",
        };

        let prompt = format!(
            r"请对以下翻译进行润色，使其更加符合{to_lang}的表达习惯。

原文（{from_lang}）：
{source_text}

当前译文：
{translated_text}

润色要求：
1. {style_instruction}
2. 保持原文含义不变
3. 修正可能的语法或表达问题
4. 只返回润色后的译文，不要添加任何解释",
            to_lang = lang_name(to_lang),
            from_lang = lang_name(from_lang),
            source_text = source_text,
            translated_text = translated_text,
            style_instruction = style_instruction
        );

        self.llm_engine
            .translate(&prompt, from_lang, to_lang)
            .await
            .map_err(|e| format!("Polish failed: {e}"))
    }

    /// Extract terms from source text and existing translations
    pub async fn extract_terms(
        &self,
        texts: &[(String, String)],
        from_lang: &str,
        to_lang: &str,
    ) -> Result<Vec<AiTermEntry>, String> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let lang_name = |code: &str| -> String {
            match code {
                "zh" => "中文".to_string(),
                "en" => "English".to_string(),
                "ja" => "日本語".to_string(),
                "ko" => "한국어".to_string(),
                _ => code.to_string(),
            }
        };

        // Build pairs text
        let pairs_text: String = texts
            .iter()
            .enumerate()
            .map(|(i, (s, t))| format!("{}. {} → {}", i + 1, s, t))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"请从以下翻译对中提取专业术语，生成术语表。

翻译对（{from_lang} → {to_lang}）：
{pairs_text}

要求：
1. 识别重复出现的专业术语
2. 提供一致的翻译
3. 以JSON格式返回，格式如下：
[
  {{"source": "原文术语", "target": "译文术语", "context": "使用场景", "frequency": 出现次数, "confidence": 置信度(0-1)}}
]
4. 只返回JSON，不要添加其他内容"#,
            from_lang = lang_name(from_lang),
            to_lang = lang_name(to_lang),
            pairs_text = pairs_text
        );

        let response = self
            .llm_engine
            .translate(&prompt, from_lang, to_lang)
            .await
            .map_err(|e| format!("Term extraction failed: {e}"))?;

        // Parse JSON response
        serde_json::from_str(&response).map_err(|e| format!("Failed to parse terms: {e}"))
    }

    /// Learn translation style from user history
    pub async fn learn_style(
        &self,
        history: &[(String, String)],
        from_lang: &str,
        to_lang: &str,
    ) -> Result<TranslationStyle, String> {
        if history.len() < 3 {
            return Err("Need at least 3 translation pairs to learn style".to_string());
        }

        let lang_name = |code: &str| -> String {
            match code {
                "zh" => "中文".to_string(),
                "en" => "English".to_string(),
                "ja" => "日本語".to_string(),
                "ko" => "한국어".to_string(),
                _ => code.to_string(),
            }
        };

        // Build examples text
        let examples_text: String = history
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, (s, t))| format!("{}. {} → {}", i + 1, s, t))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"请分析以下翻译样本，总结翻译风格特征。

翻译样本（{from_lang} → {to_lang}）：
{examples_text}

请以JSON格式返回风格分析：
{{
  "vocabulary_level": "词汇难度(basic/intermediate/advanced)",
  "sentence_structure": "句式特点",
  "tone": "语气特征",
  "formality": "正式程度(formal/neutral/casual)",
  "examples": []
}}

只返回JSON，不要添加其他内容"#,
            from_lang = lang_name(from_lang),
            to_lang = lang_name(to_lang),
            examples_text = examples_text
        );

        let response = self
            .llm_engine
            .translate(&prompt, from_lang, to_lang)
            .await
            .map_err(|e| format!("Style learning failed: {e}"))?;

        serde_json::from_str(&response).map_err(|e| format!("Failed to parse style: {e}"))
    }

    /// Context-aware translation with previous translations as context
    pub async fn translate_with_context(
        &self,
        text: &str,
        from_lang: &str,
        to_lang: &str,
        context: &[(String, String)],
    ) -> Result<String, String> {
        security::validate_text_length(text, security::MAX_TRANSLATION_TEXT_LENGTH)?;

        if context.is_empty() {
            return self
                .llm_engine
                .translate(text, from_lang, to_lang)
                .await
                .map_err(|e| e.to_string());
        }

        // Build context text (last 5 translations)
        let context_text: String = context
            .iter()
            .rev()
            .take(5)
            .enumerate()
            .map(|(i, (s, t))| format!("{}. \"{}\" → \"{}\"", i + 1, s, t))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r"请翻译以下文本，参考之前的翻译保持一致性。

待翻译文本：
{text}

之前的翻译参考：
{context_text}

要求：
1. 保持与之前翻译一致的术语和风格
2. 只返回翻译结果，不要添加解释"
        );

        self.llm_engine
            .translate(&prompt, from_lang, to_lang)
            .await
            .map_err(|e| e.to_string())
    }

    /// Multi-round translation optimization
    pub async fn multi_round_translate(
        &self,
        text: &str,
        from_lang: &str,
        to_lang: &str,
        rounds: u32,
    ) -> Result<MultiRoundResult, String> {
        security::validate_text_length(text, security::MAX_TRANSLATION_TEXT_LENGTH)?;

        let num_rounds = rounds.clamp(2, 3); // 2-3 rounds
        let mut translations = Vec::new();

        // Generate multiple translations with different temperatures
        for i in 0..num_rounds {
            let temp = 0.3 + (i as f32 * 0.2); // 0.3, 0.5, 0.7
            let translation = self
                .llm_engine
                .translate_with_temperature(text, from_lang, to_lang, temp)
                .await
                .map_err(|e| format!("Round {} failed: {}", i + 1, e))?;

            translations.push(TranslationRound {
                index: i as usize,
                translation,
                quality_score: 0.0, // Will be scored later
            });
        }

        // Score and select best translation
        let best_index = self
            .select_best_translation(text, &translations, from_lang, to_lang)
            .await?;

        Ok(MultiRoundResult {
            rounds: translations.clone(),
            best_index,
            best_translation: translations[best_index].translation.clone(),
        })
    }

    /// Select the best translation from multiple rounds
    async fn select_best_translation(
        &self,
        source: &str,
        translations: &[TranslationRound],
        from_lang: &str,
        to_lang: &str,
    ) -> Result<usize, String> {
        let lang_name = |code: &str| -> String {
            match code {
                "zh" => "中文".to_string(),
                "en" => "English".to_string(),
                "ja" => "日本語".to_string(),
                "ko" => "한국어".to_string(),
                _ => code.to_string(),
            }
        };

        let candidates: String = translations
            .iter()
            .map(|t| format!("{}. {}", t.index + 1, t.translation))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r"请从以下翻译候选中选择最佳的一个。

原文（{from_lang}）：
{source}

翻译候选（{to_lang}）：
{candidates}

评价标准：
1. 准确性：是否准确传达原文含义
2. 流畅性：是否符合{to_lang}表达习惯
3. 完整性：是否遗漏信息

只返回最佳翻译的序号（1, 2, 3...），不要添加其他内容",
            from_lang = lang_name(from_lang),
            to_lang = lang_name(to_lang),
            source = source,
            candidates = candidates
        );

        let response = self
            .llm_engine
            .translate(&prompt, from_lang, to_lang)
            .await
            .map_err(|e| format!("Selection failed: {e}"))?;

        // Parse the index
        let index: usize = response
            .trim()
            .chars()
            .next()
            .and_then(|c| c.to_digit(10))
            .map_or(0, |d| d as usize - 1);

        Ok(index.min(translations.len() - 1))
    }
}
