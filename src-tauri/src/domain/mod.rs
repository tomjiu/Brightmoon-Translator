// Domain Layer - 核心业务逻辑

pub mod card;
pub mod event;
pub mod fsrs_engine;
pub mod patch_applicator;
pub mod patch_validator;

pub use event::{
    AiContent, Annotation, CardEvent, CardPatch, CardState, Etymology, Mnemonic, MnemonicType,
    PatchOperation, PersonalizedExample, Rating, Root, Scene,
};

pub use card::{BaseData, ErrorRecord, ErrorType, WordCard};

pub use fsrs_engine::{FsrsEngine, RatingIntervals, RatingPreview};
pub use patch_applicator::PatchApplicator;
pub use patch_validator::{PatchValidationError, PatchValidator};
