//! A kind of solver methods using polynomial interpolation.
//!
//! # Example
//!
//! Here following code show encoding and decoding example. ()
//!
//! ```
//! use scuttlebutt::field::F128b;
//! use rand::Rng;
//! use scuttlebutt::AesRng;
//! use preprocessing_mpsi_with_vole::solver::{Solver, VandelmondeSolver};
//! use anyhow::Result;
//! # fn try_main() -> Result<()> {
//!
//! let mut rng: AesRng = AesRng::new();
//! let set: Vec<F128b> = (0..5).map(|_| rng.gen()).collect();
//!
//! let aux = VandelmondeSolver::<F128b>::gen_aux(&mut rng)?;
//! let params = VandelmondeSolver::<F128b>::calc_params(set.len());
//!
//! let points: Vec<(F128b, F128b)> = set
//!     .iter()
//!     .map(|x| (*x, *x * *x))
//!     .collect();
//!
//! // Encoding points to vector.
//! let p: Vec<F128b> = VandelmondeSolver::encode(&mut rng, &points, aux, params)?;
//!
//! // Vector p has the information correspondig value of each x is x * x.
//!
//! // Decoding vector to corresponding values.
//! let decoded_values: Vec<F128b> = set
//!     .iter()
//!     .map(|x| VandelmondeSolver::decode(&p, *x, aux, params))
//!     .collect::<Result<_>>()?;
//!
//! let values: Vec<F128b> = points.iter().map(|(_, y)| *y).collect();
//!
//! assert_eq!(values, decoded_values);
//! # Ok(())
//! # }
//! # fn main() {
//! #     try_main().unwrap();
//! # }
//! ```
//!
//! The usage is similar to that of the Paxos solver.
//!
//! The Vandelmonde solver uses a polynomial interpolation algorithm.

use super::*;
use anyhow::Error;
use rand::{CryptoRng, Rng};
use scuttlebutt::field::{polynomial::Polynomial, FiniteField};
use scuttlebutt::AbstractChannel;
use std::marker::PhantomData;

/// Solver using polynomial interpolation.
///
/// Please look the parent document ( [crate::solver::vandelmonde] ) for usage example.
pub struct VandelmondeSolver<FF: FiniteField>(PhantomData<FF>);

/// Parameters for VandelmondeSolver.
#[derive(Clone, Copy)]
pub struct VandelmondeSolverParams(usize);

impl SolverParams for VandelmondeSolverParams {
    fn code_length(&self) -> usize {
        self.0
    }
}

impl<FF: FiniteField> Solver<FF> for VandelmondeSolver<FF> {
    type AuxInfo = ();
    type Params = VandelmondeSolverParams;

    fn gen_aux<RNG: CryptoRng + Rng>(_rng: &mut RNG) -> Result<Self::AuxInfo, Error> {
        Ok(())
    }

    fn aux_send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        _channel: &mut C,
        _rng: &mut RNG,
        _aux: Self::AuxInfo,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn aux_receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        _channel: &mut C,
        _rng: &mut RNG,
    ) -> Result<Self::AuxInfo, Error> {
        Ok(())
    }

    fn calc_params(n: usize) -> VandelmondeSolverParams {
        VandelmondeSolverParams(n)
    }

    /// Encode points to a code vector.
    ///
    /// This function take $`O(n^3)`$ where $`n`$ is set size.
    fn encode<RNG: CryptoRng + Rng>(
        _rng: &mut RNG,
        points: &[(FF, FF)],
        _aux: (),
        _params: Self::Params,
    ) -> Result<Vec<FF>, Error> {
        let Polynomial {
            constant,
            coefficients,
        } = Polynomial::interpolate(&points);

        let mut res = vec![constant];
        res.extend(coefficients);

        Ok(res)
    }

    fn decode(p: &[FF], x: FF, _aux: (), _params: Self::Params) -> Result<FF, Error> {
        let mut temp = FF::one();
        let mut sum = FF::zero();

        for coeff in p.iter() {
            sum += *coeff * temp;
            temp *= x;
        }

        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_utils::hash_f;
    use rand::distributions::{Distribution, Standard};
    use rand::Rng;
    use scuttlebutt::field::{F128b, FiniteField};
    use scuttlebutt::AesRng;

    fn create_set<F: FiniteField>(set_size: usize) -> Vec<F>
    where
        Standard: Distribution<F>,
    {
        let mut rng = AesRng::new();

        let set = (0..set_size).map(|_| rng.gen()).collect::<Vec<_>>();

        set
    }

    #[test]
    fn test_vandelmonde() {
        let set = create_set::<F128b>(10);

        let mut rng = AesRng::new();
        let aux = VandelmondeSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = VandelmondeSolver::<F128b>::calc_params(set.len());

        let points = set
            .iter()
            .map(|x| (*x, hash_f(*x).unwrap()))
            .collect::<Vec<_>>();

        let p = VandelmondeSolver::encode(&mut rng, &points, aux, params).unwrap();

        let reconstructed_ys = set
            .iter()
            .map(|x| VandelmondeSolver::decode(&p, *x, aux, params).unwrap())
            .collect::<Vec<_>>();

        let ys = points.iter().map(|(_, y)| *y).collect::<Vec<_>>();

        assert_eq!(ys, reconstructed_ys);
    }
}
