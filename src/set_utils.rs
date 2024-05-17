//! Utility functions for creating sets for the set intersection protocol.

use anyhow::{bail, Result};
use rand::distributions::{Distribution, Standard};
use rand::seq::SliceRandom;
use rand::{CryptoRng, Rng};
use scuttlebutt::field::F128b;
use scuttlebutt::serialization::CanonicalSerialize;
use scuttlebutt::Block;
use std::collections::HashSet;

/// Trait for converting u128 to a type.
pub trait FromU128 {
    /// Convert u128 to a type.
    fn from_u128(x: u128) -> Self;
}

impl FromU128 for F128b {
    fn from_u128(x: u128) -> Self {
        let b = x.to_le_bytes();
        F128b::from_bytes(&b.into()).unwrap()
    }
}

impl FromU128 for Block {
    fn from_u128(x: u128) -> Self {
        Block::from(x)
    }
}

/// Create sets for the set intersection protocol with a check that intersection size is common_size.
pub fn create_sets_with_check<T, RNG>(
    nparties: usize,
    set_size: usize,
    common_size: usize,
    rng: &mut RNG,
) -> Result<(Vec<T>, Vec<Vec<T>>)>
where
    T: FromU128 + Clone + Copy + Eq + std::hash::Hash,
    RNG: CryptoRng + Rng,
    Standard: Distribution<T>,
{
    if nparties <= 1 {
        bail!("nparties (={}) <= 1 @{}:{}", nparties, file!(), line!());
    }

    if set_size < common_size {
        bail!(
            "set_size (={}) < common_size (={}) @{}:{}",
            set_size,
            common_size,
            file!(),
            line!()
        );
    }

    let common = (0..common_size).map(|_| rng.gen::<T>()).collect::<Vec<_>>();

    let mut sets = (0..nparties)
        .map(|i| {
            let mut set = HashSet::<T>::from_iter(common.clone().into_iter());

            let mut counter: usize = 0;
            while set.len() < set_size {
                if counter >= nparties {
                    break;
                }

                if i == counter {
                    counter += 1;
                    continue;
                }

                let x = T::from_u128(counter as u128);

                set.insert(x);

                counter += 1;
            }

            while set.len() < set_size {
                set.insert(rng.gen::<T>());
            }
            Ok(set.into_iter().collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>()?;

    let set0 = sets[0].clone();
    let common = sets.iter().skip(1).fold(set0, |acc, set| {
        acc.into_iter()
            .filter(|x| set.contains(x))
            .collect::<Vec<_>>()
    });

    for set in sets.iter_mut() {
        set.shuffle(rng);
    }

    Ok((common, sets))
}

/// Create sets for the set intersection protocol without checks.
/// It is useful to create big sets for performance testing.
pub fn create_sets_without_check<T, RNG>(
    nparties: usize,
    set_size: usize,
    common_size: usize,
    rng: &mut RNG,
) -> Result<(Vec<T>, Vec<Vec<T>>)>
where
    T: FromU128 + Clone + Copy + Eq + std::hash::Hash,
    RNG: CryptoRng + Rng,
    Standard: Distribution<T>,
{
    if nparties <= 1 {
        bail!("nparties (={}) <= 1 @{}:{}", nparties, file!(), line!());
    }

    if set_size < common_size {
        bail!(
            "set_size (={}) < common_size (={}) @{}:{}",
            set_size,
            common_size,
            file!(),
            line!()
        );
    }

    let common = (0..common_size).map(|_| rng.gen::<T>()).collect::<Vec<_>>();

    let mut sets = (0..nparties)
        .map(|i| {
            let mut set = HashSet::<T>::from_iter(common.clone().into_iter());

            let mut counter: usize = 0;
            while set.len() < set_size {
                if counter >= nparties {
                    break;
                }

                if i == counter {
                    counter += 1;
                    continue;
                }

                let x = T::from_u128(counter as u128);

                set.insert(x);

                counter += 1;
            }

            while set.len() < set_size {
                set.insert(rng.gen::<T>());
            }
            Ok(set.into_iter().collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>()?;

    for set in sets.iter_mut() {
        set.shuffle(rng);
    }

    Ok((common, sets))
}

/// Create sets for the set intersection protocol so that intersection of sets is random size.
pub fn create_sets_random<T, RNG>(
    nparties: usize,
    set_size: usize,
    rng: &mut RNG,
) -> Result<(Vec<T>, Vec<Vec<T>>)>
where
    T: FromU128 + Clone + Copy + Eq + std::hash::Hash,
    RNG: CryptoRng + Rng,
    Standard: Distribution<T>,
{
    let common_size = rng.gen_range(0..set_size);

    create_sets_without_check(nparties, set_size, common_size, rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scuttlebutt::AesRng;

    #[test]
    fn test_small() {
        let mut rng = AesRng::new();

        let (common, sets): (Vec<Block>, Vec<Vec<Block>>) =
            create_sets_with_check(3, 10, 5, &mut rng).unwrap();

        dbg!(common.clone());
        dbg!(sets.clone());

        assert_eq!(sets.len(), 3);
        assert_eq!(sets[0].len(), 10);
        assert_eq!(sets[1].len(), 10);
        assert_eq!(sets[2].len(), 10);

        for x in common.iter() {
            assert!(sets[0].contains(x));
            assert!(sets[1].contains(x));
            assert!(sets[2].contains(x));
        }
    }

    #[test]
    fn test_big_with_check() {
        let mut rng = AesRng::new();

        let (common, sets): (Vec<F128b>, Vec<Vec<F128b>>) =
            create_sets_with_check(5, 1 << 6, 1 << 3, &mut rng).unwrap();

        for x in common.iter() {
            for set in sets.iter() {
                assert!(set.contains(x));
            }
        }
    }

    #[test]
    fn test_big_without_check() {
        let mut rng = AesRng::new();

        let (_common, _sets): (Vec<F128b>, Vec<Vec<F128b>>) =
            create_sets_without_check(5, 1 << 20, 1 << 10, &mut rng).unwrap();
    }
}
