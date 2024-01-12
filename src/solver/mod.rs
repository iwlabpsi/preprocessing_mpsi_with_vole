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

pub trait SolverParams {
    fn code_length(&self) -> usize;
}

pub trait Solver<FF: FiniteField> {
    type AuxInfo: 'static + Clone + Copy + Send;
    type Params: 'static + Clone + Copy + Send + SolverParams;

    fn gen_aux<RNG: CryptoRng + Rng>(rng: &mut RNG) -> Result<Self::AuxInfo, Error>;

    fn aux_send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        aux: Self::AuxInfo,
    ) -> Result<(), Error>;

    fn aux_receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
    ) -> Result<Self::AuxInfo, Error>;

    fn calc_params(n: usize) -> Self::Params;

    fn encode<RNG: CryptoRng + Rng>(
        rng: &mut RNG,
        points: &[(FF, FF)],
        aux: Self::AuxInfo,
        params: Self::Params,
    ) -> Result<Vec<FF>, Error>;

    fn decode(p: &[FF], x: FF, aux: Self::AuxInfo, params: Self::Params) -> Result<FF, Error>;
}
