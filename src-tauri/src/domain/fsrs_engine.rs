// FSRS Engine - 复习调度算法（纯 Rust 实现）
// 基于 FSRS-4.5 算法

use crate::domain::{CardState, Rating};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

/// FSRS 引擎
pub struct FsrsEngine {
    /// 参数权重（默认 FSRS-4.5 参数）
    w: [f64; 17],
}

impl FsrsEngine {
    /// 创建默认引擎（FSRS-4.5 默认参数）
    pub fn new() -> Self {
        Self {
            w: [
                0.4, 0.6, 2.4, 5.8, 4.93, 0.94, 0.86, 0.01, 1.49, 0.14, 0.94, 2.18, 0.05, 0.34,
                1.26, 0.29, 2.61,
            ],
        }
    }

    /// 使用自定义参数创建
    pub fn with_params(params: [f64; 17]) -> Self {
        Self { w: params }
    }

    /// 计算下次复习
    pub fn schedule_review(
        &self,
        current_state: &CardState,
        rating: Rating,
        review_time: DateTime<Utc>,
    ) -> Result<CardState> {
        let elapsed_days = if let Some(last_review) = current_state.last_review {
            let last = DateTime::from_timestamp(last_review, 0).unwrap();
            (review_time - last).num_days().max(0) as u32
        } else {
            0
        };

        let (new_stability, new_difficulty) = if current_state.reps == 0 {
            // 新卡片
            self.init_stability_difficulty(rating)
        } else {
            // 已学习卡片
            self.next_stability_difficulty(current_state, rating, elapsed_days)
        };

        let scheduled_days = self.next_interval(new_stability);
        let next_review_time = review_time + Duration::days(scheduled_days as i64);

        let (new_reps, new_lapses) = match rating {
            Rating::Again => (current_state.reps + 1, current_state.lapses + 1),
            _ => (current_state.reps + 1, current_state.lapses),
        };

        Ok(CardState {
            stability: new_stability,
            difficulty: new_difficulty,
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

    /// 初始化稳定性和难度（新卡片）
    fn init_stability_difficulty(&self, rating: Rating) -> (f64, f64) {
        let stability = match rating {
            Rating::Again => self.w[0],
            Rating::Hard => self.w[1],
            Rating::Good => self.w[2],
            Rating::Easy => self.w[3],
        };

        let difficulty = self.w[4] - (rating as i32 as f64 - 3.0) * self.w[5];
        let difficulty = difficulty.clamp(1.0, 10.0);

        (stability, difficulty)
    }

    /// 计算下一个稳定性和难度（已学习卡片）
    fn next_stability_difficulty(
        &self,
        state: &CardState,
        rating: Rating,
        elapsed_days: u32,
    ) -> (f64, f64) {
        let retrievability = self.forgetting_curve(elapsed_days, state.stability);

        let new_difficulty = if rating == Rating::Again {
            state.difficulty
        } else {
            let new_d = state.difficulty - self.w[6] * (rating as i32 as f64 - 3.0);
            new_d.clamp(1.0, 10.0)
        };

        let new_stability = match rating {
            Rating::Again => {
                self.w[11]
                    * state.difficulty.powf(-self.w[12])
                    * ((state.stability + 1.0).powf(self.w[13]) - 1.0)
                    * (1.0 - retrievability).exp()
            },
            Rating::Hard => {
                state.stability
                    * (1.0
                        + (self.w[7] * (11.0 - new_difficulty) * state.stability.powf(-self.w[8]))
                            .exp()
                            * (1.0 - retrievability))
            },
            Rating::Good => {
                state.stability
                    * (1.0
                        + (self.w[7] * (11.0 - new_difficulty) * state.stability.powf(-self.w[8]))
                            .exp()
                            * (1.0 - retrievability))
            },
            Rating::Easy => {
                state.stability
                    * (1.0
                        + (self.w[9] * (11.0 - new_difficulty) * state.stability.powf(-self.w[10]))
                            .exp()
                            * (1.0 - retrievability))
            },
        };

        (new_stability.max(0.01), new_difficulty)
    }

    /// 计算下一个间隔（天数）
    fn next_interval(&self, stability: f64) -> u32 {
        let interval = (stability * 9.0 * (1.0 / 0.9 - 1.0)).round();
        interval.max(1.0) as u32
    }

    /// 遗忘曲线（计算记忆保持率）
    pub fn forgetting_curve(&self, elapsed_days: u32, stability: f64) -> f64 {
        (1.0 + (elapsed_days as f64) / (9.0 * stability)).powf(-1.0)
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
}

impl Default for FsrsEngine {
    fn default() -> Self {
        Self::new()
    }
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

        // 第一次复习：Good
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

        // 第一次：Good
        let state1 = engine.schedule_review(&initial, Rating::Good, now).unwrap();
        assert_eq!(state1.lapses, 0);

        // 第二次：Again（忘记）
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

        // 验证间隔递增：Again < Hard < Good < Easy
        assert!(intervals.again <= intervals.hard);
        assert!(intervals.hard <= intervals.good);
        assert!(intervals.good <= intervals.easy);
    }

    #[test]
    fn test_should_review() {
        let engine = FsrsEngine::new();
        let mut state = engine.initial_state();

        // 新卡片应该立即复习
        assert!(engine.should_review(&state));

        // 设置未来的复习时间
        state.next_review = (Utc::now() + Duration::days(1)).timestamp();
        assert!(!engine.should_review(&state));

        // 设置过去的复习时间
        state.next_review = (Utc::now() - Duration::days(1)).timestamp();
        assert!(engine.should_review(&state));
    }

    #[test]
    fn test_forgetting_curve() {
        let engine = FsrsEngine::new();
        let stability = 10.0;

        // 刚复习完，记忆保持率接近 1.0
        let retention_0 = engine.forgetting_curve(0, stability);
        assert!(retention_0 > 0.99);

        // 经过一段时间，记忆保持率下降
        let retention_10 = engine.forgetting_curve(10, stability);
        assert!(retention_10 < retention_0);
        assert!(retention_10 > 0.5);
    }

    #[test]
    fn test_difficulty_progression() {
        let engine = FsrsEngine::new();
        let initial = engine.initial_state();
        let now = Utc::now();

        // Good 评分，难度应该适中
        let state_good = engine.schedule_review(&initial, Rating::Good, now).unwrap();
        assert!(state_good.difficulty > 1.0);
        assert!(state_good.difficulty < 10.0);

        // Easy 评分，难度应该更低
        let state_easy = engine.schedule_review(&initial, Rating::Easy, now).unwrap();
        assert!(state_easy.difficulty < state_good.difficulty);

        // Hard 评分，难度应该更高
        let state_hard = engine.schedule_review(&initial, Rating::Hard, now).unwrap();
        assert!(state_hard.difficulty > state_good.difficulty);
    }
}
