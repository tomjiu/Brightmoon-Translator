// FSRS Engine - wraps official fsrs crate (FSRS-6 scheduler)

use crate::domain::{CardState, Rating};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use fsrs::{FSRS, ItemState, MemoryState};

/// FSRS 引擎
pub struct FsrsEngine {
    inner: FSRS,
    /// Cached weights for `get_params` (first 17 of default / custom)
    w: [f64; 17],
    desired_retention: f32,
}

impl FsrsEngine {
    /// 创建默认引擎
    pub fn new() -> Self {
        let inner = FSRS::default();
        Self {
            inner,
            w: default_w17(),
            desired_retention: 0.9,
        }
    }

    /// 使用自定义参数创建（不足则用默认补齐；多余截断到 17 供 `get_params`）
    pub fn with_params(params: [f64; 17]) -> Self {
        let f32_params: Vec<f32> = params.iter().map(|&x| x as f32).collect();
        let inner = FSRS::new(&f32_params).unwrap_or_default();
        Self {
            inner,
            w: params,
            desired_retention: 0.9,
        }
    }

    /// 计算下次复习
    pub fn schedule_review(
        &self,
        current_state: &CardState,
        rating: Rating,
        review_time: DateTime<Utc>,
    ) -> Result<CardState> {
        let elapsed_days = if let Some(last_review) = current_state.last_review {
            let last = DateTime::from_timestamp(last_review, 0).unwrap_or(review_time);
            (review_time - last).num_days().max(0) as u32
        } else {
            0
        };

        let prev = if current_state.reps == 0 || current_state.stability <= 0.0 {
            None
        } else {
            Some(MemoryState {
                stability: current_state.stability as f32,
                difficulty: current_state.difficulty as f32,
            })
        };

        let next = self
            .inner
            .next_states(prev, self.desired_retention, elapsed_days)
            .map_err(|e| anyhow!("FSRS next_states failed: {e}"))?;

        let item = pick_rating(&next, rating);
        let scheduled_days = item.interval.round().max(1.0) as u32;
        let next_review_time = review_time + Duration::days(i64::from(scheduled_days));

        let (new_reps, new_lapses) = match rating {
            Rating::Again => (current_state.reps + 1, current_state.lapses + 1),
            _ => (current_state.reps + 1, current_state.lapses),
        };

        Ok(CardState {
            stability: f64::from(item.memory.stability),
            difficulty: f64::from(item.memory.difficulty),
            elapsed_days,
            scheduled_days,
            reps: new_reps,
            lapses: new_lapses,
            last_review: Some(review_time.timestamp()),
            next_review: next_review_time.timestamp(),
        })
    }

    /// 获取初始状态
    pub fn initial_state(&self) -> CardState {
        CardState {
            stability: 0.0,
            difficulty: 0.0,
            elapsed_days: 0,
            scheduled_days: 0,
            reps: 0,
            lapses: 0,
            last_review: None,
            next_review: Utc::now().timestamp(),
        }
    }

    /// 预览不同评分的结果
    pub fn preview_ratings(
        &self,
        current_state: &CardState,
        review_time: DateTime<Utc>,
    ) -> Result<RatingPreview> {
        let again = self.schedule_review(current_state, Rating::Again, review_time)?;
        let hard = self.schedule_review(current_state, Rating::Hard, review_time)?;
        let good = self.schedule_review(current_state, Rating::Good, review_time)?;
        let easy = self.schedule_review(current_state, Rating::Easy, review_time)?;

        Ok(RatingPreview {
            again,
            hard,
            good,
            easy,
        })
    }

    /// 遗忘曲线（计算记忆保持率）— FSRS-6 style with decay ≈ 0.5 for display
    pub fn forgetting_curve(&self, elapsed_days: u32, stability: f64) -> f64 {
        if stability <= 0.0 {
            return 0.0;
        }
        let factor = (0.9f64).powf(1.0 / -0.5) - 1.0;
        (1.0 + factor * f64::from(elapsed_days) / stability).powf(-0.5)
    }

    /// 是否应该复习
    pub fn should_review(&self, state: &CardState) -> bool {
        let now = Utc::now().timestamp();
        now >= state.next_review
    }

    /// 获取逾期天数
    pub fn overdue_days(&self, state: &CardState) -> i64 {
        let now = Utc::now().timestamp();
        if now < state.next_review {
            0
        } else {
            (now - state.next_review) / 86400
        }
    }

    /// 获取待复习卡片（距下次复习时间）
    pub fn days_until_review(&self, state: &CardState) -> i64 {
        let now = Utc::now().timestamp();
        (state.next_review - now) / 86400
    }

    /// 获取当前参数
    pub fn get_params(&self) -> &[f64; 17] {
        &self.w
    }
}

impl Default for FsrsEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn pick_rating(next: &fsrs::NextStates, rating: Rating) -> ItemState {
    match rating {
        Rating::Again => next.again.clone(),
        Rating::Hard => next.hard.clone(),
        Rating::Good => next.good.clone(),
        Rating::Easy => next.easy.clone(),
    }
}

fn default_w17() -> [f64; 17] {
    // First 17 of FSRS default weights (display / with_params compat)
    [
        0.4, 0.6, 2.4, 5.8, 4.93, 0.94, 0.86, 0.01, 1.49, 0.14, 0.94, 2.18, 0.05, 0.34, 1.26, 0.29,
        2.61,
    ]
}

/// 评分预览
#[derive(Debug, Clone)]
pub struct RatingPreview {
    pub again: CardState,
    pub hard: CardState,
    pub good: CardState,
    pub easy: CardState,
}

impl RatingPreview {
    /// 获取下次复习时间（天数）
    pub fn intervals(&self) -> RatingIntervals {
        RatingIntervals {
            again: self.again.scheduled_days,
            hard: self.hard.scheduled_days,
            good: self.good.scheduled_days,
            easy: self.easy.scheduled_days,
        }
    }
}

/// 评分间隔
#[derive(Debug, Clone, Copy)]
pub struct RatingIntervals {
    pub again: u32,
    pub hard: u32,
    pub good: u32,
    pub easy: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let engine = FsrsEngine::new();
        let state = engine.initial_state();

        assert_eq!(state.reps, 0);
        assert_eq!(state.lapses, 0);
        assert!(state.last_review.is_none());
    }

    #[test]
    fn test_first_review() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let now = Utc::now();

        let after_good = engine.schedule_review(&initial, Rating::Good, now).unwrap();

        assert_eq!(after_good.reps, 1);
        assert!(after_good.stability > 0.0);
        assert!(after_good.scheduled_days > 0);
        assert!(after_good.last_review.is_some());
    }

    #[test]
    fn test_lapse() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let now = Utc::now();

        let state1 = engine.schedule_review(&initial, Rating::Good, now).unwrap();
        assert_eq!(state1.lapses, 0);

        let state2 = engine
            .schedule_review(&state1, Rating::Again, now + Duration::days(1))
            .unwrap();
        assert_eq!(state2.lapses, 1);
    }

    #[test]
    fn test_preview_ratings() {
        let engine = FsrsEngine::new();
        let state = engine.initial_state();
        let now = Utc::now();

        let preview = engine.preview_ratings(&state, now).unwrap();
        let intervals = preview.intervals();

        assert!(intervals.again <= intervals.hard);
        assert!(intervals.hard <= intervals.good);
        assert!(intervals.good <= intervals.easy);
    }

    #[test]
    fn test_should_review() {
        let engine = FsrsEngine::new();
        let mut state = engine.initial_state();

        assert!(engine.should_review(&state));

        state.next_review = (Utc::now() + Duration::days(1)).timestamp();
        assert!(!engine.should_review(&state));

        state.next_review = (Utc::now() - Duration::days(1)).timestamp();
        assert!(engine.should_review(&state));
    }

    #[test]
    fn test_forgetting_curve() {
        let engine = FsrsEngine::new();
        let stability = 10.0;

        let retention_0 = engine.forgetting_curve(0, stability);
        assert!(retention_0 > 0.99);

        let retention_10 = engine.forgetting_curve(10, stability);
        assert!(retention_10 < retention_0);
        assert!(retention_10 > 0.5);
    }

    #[test]
    fn test_difficulty_progression() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let now = Utc::now();

        let state_good = engine.schedule_review(&initial, Rating::Good, now).unwrap();
        assert!(state_good.difficulty > 1.0);
        assert!(state_good.difficulty < 10.0);

        let state_easy = engine.schedule_review(&initial, Rating::Easy, now).unwrap();
        assert!(state_easy.difficulty < state_good.difficulty);

        let state_hard = engine.schedule_review(&initial, Rating::Hard, now).unwrap();
        assert!(state_hard.difficulty > state_good.difficulty);
    }

    #[test]
    fn hard_stability_differs_from_good_after_review() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let t0 = Utc::now();
        let learned = engine.schedule_review(&initial, Rating::Good, t0).unwrap();
        let t1 = t0 + Duration::days(learned.scheduled_days.max(1) as i64);
        let hard = engine.schedule_review(&learned, Rating::Hard, t1).unwrap();
        let good = engine.schedule_review(&learned, Rating::Good, t1).unwrap();
        assert!(
            (hard.stability - good.stability).abs() > 1e-6,
            "Hard and Good must produce different stability (got hard={} good={})",
            hard.stability,
            good.stability
        );
        assert!(hard.scheduled_days <= good.scheduled_days);
    }

    #[test]
    fn again_increases_difficulty_on_review() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let t0 = Utc::now();
        let learned = engine.schedule_review(&initial, Rating::Good, t0).unwrap();
        let d0 = learned.difficulty;
        let t1 = t0 + Duration::days(learned.scheduled_days.max(1) as i64);
        let after_again = engine.schedule_review(&learned, Rating::Again, t1).unwrap();
        assert!(
            after_again.difficulty > d0,
            "Again must increase difficulty (before={} after={})",
            d0,
            after_again.difficulty
        );
        assert_eq!(after_again.lapses, learned.lapses + 1);
    }

    #[test]
    fn rating_order_intervals_strict_or_nondecreasing() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let t0 = Utc::now();
        let learned = engine.schedule_review(&initial, Rating::Good, t0).unwrap();
        let t1 = t0 + Duration::days(learned.scheduled_days.max(1) as i64);
        let preview = engine.preview_ratings(&learned, t1).unwrap();
        let i = preview.intervals();
        assert!(i.again <= i.hard);
        assert!(i.hard <= i.good);
        assert!(i.good <= i.easy);
        assert!(i.hard < i.good || i.again < i.hard || i.good < i.easy);
    }
}
