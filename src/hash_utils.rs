use anyhow::{Context, Result};
use scuttlebutt::field::FiniteField as FF;
use sha2::{Digest, Sha256};
use typenum::marker_traits::Unsigned;

// H: F x F -> F
#[inline]
pub fn hash<F: FF>(x: F, y: F) -> Result<F> {
    let mut hasher = Sha256::new();
    hasher.update(x.to_bytes());
    hasher.update(y.to_bytes());
    let res = hasher.finalize();
    let slc = res.as_slice();
    let len = F::ByteReprLen::to_usize();
    let byt = (&slc[..len]).as_ref().into();
    Ok(F::from_bytes(byt).with_context(|| format!("@{}:{}", file!(), line!()))?)
}

// H^F: F x F -> F
#[inline]
pub fn hash_f<F: FF>(x: F) -> Result<F> {
    let mut hasher = Sha256::new();
    hasher.update(x.to_bytes());
    let res = hasher.finalize();
    let slc = res.as_slice();
    let len = F::ByteReprLen::to_usize();
    let byt = (&slc[..len]).as_ref().into();
    Ok(F::from_bytes(byt).with_context(|| format!("@{}:{}", file!(), line!()))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use scuttlebutt::field::F128b;
    use scuttlebutt::serialization::CanonicalSerialize;
    use scuttlebutt::AesRng;

    #[test]
    fn test_hash() {
        let mut rng = AesRng::new();
        let x: F128b = rng.gen();
        let y: F128b = rng.gen();

        let h = hash(x, y).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(x.to_bytes());
        hasher.update(y.to_bytes());
        let res = hasher.finalize();
        let slc = res.as_slice();
        let len = <F128b as CanonicalSerialize>::ByteReprLen::to_usize();
        let byt = (&slc[..len]).as_ref().into();
        let h2 = F128b::from_bytes(byt).unwrap();

        dbg!(x, y, h);

        assert_eq!(h, h2);
    }

    #[test]
    fn test_hash_f() {
        let mut rng = AesRng::new();
        let x: F128b = rng.gen();

        let h = hash_f(x).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(x.to_bytes());
        let res = hasher.finalize();
        let slc = res.as_slice();
        let len = <F128b as CanonicalSerialize>::ByteReprLen::to_usize();
        let byt = (&slc[..len]).as_ref().into();
        let h2 = F128b::from_bytes(byt).unwrap();

        dbg!(x, h);

        assert_eq!(h, h2);
    }
}
