//! VOLE (Vector Oblivious Linear Evaluation) module.
//!
//! VOLE is tuples of vector of correlated randomness shared between two parties.
//!
//! One party has vectors $`\bm{A}, \bm{C} \in \mathbb{F}^m`$, and another party has vectors $`\bm{B} \in \mathbb{F}^m`$ and scalar $`\Delta \in \mathbb{F}`$
//! where $`m`$ is the length of vectors and determined by [solver algorithm](crate::solver) in light of the size of sets.
//!
//! VOLE vectors satisfy the following properties:
//!
//! ```math
//! \bm{C} = \bm{A} \Delta + \bm{B}
//! ```
//!
//! VOLE vectors are used to mask code vectors. e.g. $`\bm{P} + \bm{A} \in \mathbb{F}^m`$ is masked code vector from $`\bm{P} \in \mathbb{F}^m`$.
//!
//! For more detail (or if you want to know purpose of masking), see the following paper:
//!
//! - [VOLE-PSI: Fast OPRF and Circuit-PSI from Vector-OLE](https://eprint.iacr.org/2021/266)

use anyhow::Error;
pub use ocelot::svole::wykw::{
    LpnParams, LPN_EXTEND_LARGE, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_LARGE,
    LPN_SETUP_MEDIUM, LPN_SETUP_SMALL,
};
use rand::{CryptoRng, Rng};
use scuttlebutt::channel::AbstractChannel;
use scuttlebutt::field::FiniteField as FF;

pub mod lpn_based;
pub use lpn_based::{LPNVoleReceiver, LPNVoleSender};
pub mod ot_based;
pub use ot_based::{OtVoleReceiver, OtVoleSender};

/// Trait for VOLE sender.
pub trait VoleShareForSender<F: FF>: Clone + Copy {
    /// Receive $`\Delta \in \mathbb{F}, \bm{B} \in \mathbb{F}^m`$
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(F, Vec<F>), Error>;
}

/// Trait for VOLE receiver.
pub trait VoleShareForReceiver<F: FF>: Clone + Copy {
    /// Receive $`\bm{A}, \bm{C} \in \mathbb{F}^m`$
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(Vec<F>, Vec<F>), Error>;
}
