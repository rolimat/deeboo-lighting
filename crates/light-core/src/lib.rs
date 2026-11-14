//! Pure, deterministic core: no I/O, no real time.
pub mod builtin;
pub mod clock;
pub mod detect;
pub mod engine;
pub mod generator;
pub mod library;
#[cfg(feature = "serde")]
pub mod migrate;
pub mod model;
pub mod moments;
pub mod paint;
pub mod parts;
pub mod patterns;
pub mod rotation;
pub mod scene;
pub mod scheduler;
