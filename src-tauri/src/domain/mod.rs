// Domain Layer - 核心业务逻辑

pub mod card;
pub mod event;
pub mod fsrs_engine;
pub mod learning_plan;
pub mod patch_applicator;
pub mod patch_validator;
pub mod state_machine;

pub use event::{
    AiContent, Annotation, CardEvent, CardPatch, CardState, Etymology, Mnemonic, MnemonicType,
    PatchOperation, PersonalizedExample, Rating, Root, Scene, WordFamilyItem,
};

pub use card::{BaseData, ErrorRecord, ErrorType, WordCard};

pub use fsrs_engine::{FsrsEngine, RatingIntervals, RatingPreview};
pub use learning_plan::{
    CreatePlanRequest, LearningPlan, PlanProgressStats, PlanStatus, PlanSummary, PlanType,
    PlanWord, PresetWordlist, TargetExam,
};
pub use patch_applicator::PatchApplicator;
pub use patch_validator::{PatchValidationError, PatchValidator};
pub use state_machine::{LearningPhase, LearningState, NextAction, OptimizeTrigger, StateMachine};
