//! Module that re-exports [popsicle::kmprt] module and provides its multithread optimized implementaion.

pub mod mt;
pub use popsicle::kmprt::{Receiver, Sender};
