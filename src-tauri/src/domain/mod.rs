// Domain Layer - 核心业务逻辑

pub mod card;
pub mod event;

pub use event::{
    AiContent, Annotation, CardEvent, CardPatch, CardState, Etymology, Mnemonic, MnemonicType,
    PatchOperation, PersonalizedExample, Rating, Root, Scene,
};

pub use card::{BaseData, ErrorRecord, ErrorType, WordCard};
