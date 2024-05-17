//! Solver module.
//!
//! Solvers in this library is somewhat similar to simultaneous equation solver.
//!
//! A solver is used in online phase to encode points into code vector and decode code vector into points.
//!
//! Thanks to encoding, party can exchange sets information (e.g. Is an element included the set?) using masking with VOLE.
//!
//! For more detail, see the following papers:
//!
//! - [PSI from PaXoS: Fast, Malicious Private Set Intersection](https://eprint.iacr.org/2020/193)
//! - [VOLE-PSI: Fast OPRF and Circuit-PSI from Vector-OLE](https://eprint.iacr.org/2021/266)
//!
//! Or, the implementation source code of [PaxosSolver].

use anyhow::Error;
use rand::{CryptoRng, Rng};
use scuttlebutt::field::FiniteField;
use scuttlebutt::AbstractChannel;
pub mod vandelmode;
pub use vandelmode::VandelmondeSolver;
mod gaussian_eliminations;
pub mod paxos;
pub use paxos::PaxosSolver;
// mod lu_decomp;

/// Trait for solver parameters.
/// Code length varies depending on the solver.
pub trait SolverParams {
    /// return code length of the solver.
    fn code_length(&self) -> usize;
}

/// Trait for the solver.
pub trait Solver<FF: FiniteField> {
    /// Auxillary information for the solver. e.g. shared seeds to create random matrix used in [PaxosSolver].
    /// Auxillary information is decided according to set size.
    type AuxInfo: 'static + Clone + Copy + Send;
    /// Parameters for the solver. e.g. left part length and right part length in code vectors used in [PaxosSolver].
    /// Parameters are decided by the solver on runtime.
    type Params: 'static + Clone + Copy + Send + SolverParams;

    /// Generate auxillary information for the solver.
    fn gen_aux<RNG: CryptoRng + Rng>(rng: &mut RNG) -> Result<Self::AuxInfo, Error>;

    /// Send auxillary information for another party.
    fn aux_send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        aux: Self::AuxInfo,
    ) -> Result<(), Error>;

    /// Receive auxillary information from another party.
    fn aux_receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
    ) -> Result<Self::AuxInfo, Error>;

    /// Calculate parameters for the solver according to set size.
    fn calc_params(n: usize) -> Self::Params;

    /// Encode points $`(\in (\mathbb{F} \times \mathbb{F})^n)`$ into code vector $`P \in \mathbb{F}^m`$.
    fn encode<RNG: CryptoRng + Rng>(
        rng: &mut RNG,
        points: &[(FF, FF)],
        aux: Self::AuxInfo,
        params: Self::Params,
    ) -> Result<Vec<FF>, Error>;

    /// Decode code vector $`P`$ and value $`x \in \mathbb{F}`$ into value $`y \in \mathbb{F}`$ which corresponds to $`x`$.
    fn decode(p: &[FF], x: FF, aux: Self::AuxInfo, params: Self::Params) -> Result<FF, Error>;
}
