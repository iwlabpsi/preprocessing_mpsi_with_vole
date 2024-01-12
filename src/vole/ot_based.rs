use super::{VoleShareForReceiver, VoleShareForSender};
use crate::set_utils::FromU128;
use anyhow::{Context, Error, Result};
use generic_array::GenericArray;
use itertools::Itertools;
use ocelot::ot::{Receiver as OtReceiver, Sender as OtSender};
use rand::distributions::{Distribution, Standard};
use rand::{CryptoRng, Rng};
use scuttlebutt::channel::AbstractChannel;
use scuttlebutt::field::FiniteField as FF;
use scuttlebutt::serialization::CanonicalSerialize;
use scuttlebutt::Block;
use std::marker::PhantomData;

fn bytes2block(bytes: &[u8]) -> Block {
    let mut b = [0u8; 16];
    b.copy_from_slice(bytes);
    Block::from(b)
}

pub struct OtVoleSender<F, const F_LENGTH: usize, OT>(PhantomData<(F, OT)>)
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtSender,
    Standard: Distribution<F>;

impl<F, const F_LENGTH: usize, OT> Clone for OtVoleSender<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtSender,
    Standard: Distribution<F>,
{
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}
impl<F, const F_LENGTH: usize, OT> Copy for OtVoleSender<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtSender,
    Standard: Distribution<F>,
{
}

impl<F, const F_LENGTH: usize, OT> OtVoleSender<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtSender,
    Standard: Distribution<F>,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<F, const F_LENGTH: usize, OT> VoleShareForSender<F> for OtVoleSender<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtSender,
    Standard: Distribution<F>,
    OT::Msg: From<Block>,
{
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(F, Vec<F>), Error> {
        let delta: F = rng.gen();

        let mut delta_2pows: [F; F_LENGTH] = [F::zero(); F_LENGTH];
        let mut delta_2pow = delta;
        let two = F::from_u128(2);
        for i in 0..F_LENGTH {
            delta_2pows[i] = delta_2pow;
            delta_2pow *= two;
        }

        let mut b_vec: Vec<F> = Vec::with_capacity(m);
        let mut inputs: Vec<(OT::Msg, OT::Msg)> = Vec::with_capacity(m * F_LENGTH);
        for _ in 0..m {
            let mut b = F::zero();
            for i in 0..F_LENGTH {
                let rho: F = rng.gen();
                b += rho;
                let left = bytes2block(&rho.to_bytes());
                let right = bytes2block(&(rho + delta_2pows[i]).to_bytes());
                inputs.push((left.into(), right.into()));
            }
            b_vec.push(b);
        }

        let mut ot = OT::init(channel, rng).with_context(|| format!("@{}:{}", file!(), line!()))?;
        ot.send(channel, &inputs, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok((delta, b_vec))
    }
}

pub struct OtVoleReceiver<F, const F_LENGTH: usize, OT>(PhantomData<(F, OT)>)
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtReceiver,
    Standard: Distribution<F>;

impl<F, const F_LENGTH: usize, OT> Clone for OtVoleReceiver<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtReceiver,
    Standard: Distribution<F>,
{
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<F, const F_LENGTH: usize, OT> Copy for OtVoleReceiver<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtReceiver,
    Standard: Distribution<F>,
{
}

impl<F, const F_LENGTH: usize, OT> OtVoleReceiver<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtReceiver,
    Standard: Distribution<F>,
{
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<F, const F_LENGTH: usize, OT> VoleShareForReceiver<F> for OtVoleReceiver<F, F_LENGTH, OT>
where
    F: FF + FromU128 + CanonicalSerialize,
    OT: OtReceiver,
    Standard: Distribution<F>,
    <F as CanonicalSerialize>::ByteReprLen: generic_array::ArrayLength<u8>,
    OT::Msg: From<Block>,
{
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(Vec<F>, Vec<F>), Error> {
        let mut a_vec = Vec::with_capacity(m);
        let mut inputs: Vec<bool> = Vec::with_capacity(m * F_LENGTH);

        for _ in 0..m {
            let a = rng.gen::<u128>();
            if a == 0_u128 {
                continue;
            }

            for i in 0..F_LENGTH {
                let a_bit = (a >> i) & 1 == 1;
                inputs.push(a_bit);
            }

            a_vec.push(F::from_u128(a));
        }

        let mut ot = OT::init(channel, rng).with_context(|| format!("@{}:{}", file!(), line!()))?;
        let rhos = ot
            .receive(channel, &inputs, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let c_vec = rhos
            .into_iter()
            .chunks(F_LENGTH)
            .into_iter()
            .map(|rho| {
                let res = rho
                    .into_iter()
                    .map(|mut msg| {
                        let v = msg.as_mut();
                        let ga =
                            GenericArray::<u8, <F as CanonicalSerialize>::ByteReprLen>::from_slice(
                                v,
                            );
                        let f = F::from_bytes(ga)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        Ok(f)
                    })
                    .collect::<Result<Vec<_>, Error>>()?
                    .into_iter()
                    .sum();
                Ok(res)
            })
            .collect::<Result<Vec<_>>>()?;

        assert!(c_vec.len() == m);

        Ok((a_vec, c_vec))
    }
}

#[cfg(test)]
mod tests {
    use crate::channel_utils::{read_vec_f, write_vec_f};

    use super::*;
    use ocelot::ot::{AlszReceiver as OtReceiver, AlszSender as OtSender};
    use scuttlebutt::{field::F128b, AesRng, Channel};
    use std::io::{BufReader, BufWriter};
    use std::os::unix::net::UnixStream;

    fn test_vole_share_base(vole_size: usize) {
        let (sender, receiver) = UnixStream::pair().unwrap();
        let handle = std::thread::spawn(move || {
            let mut rng = AesRng::new();
            let reader = BufReader::new(sender.try_clone().unwrap());
            let writer = BufWriter::new(sender);
            let mut channel = Channel::new(reader, writer);

            let mut vole_sender = OtVoleSender::<F128b, 128, OtSender>::new();
            let (delta, out) = vole_sender
                .receive(&mut channel, &mut rng, vole_size)
                .with_context(|| format!("@{}:{}", file!(), line!()))
                .unwrap();

            channel
                .write_serializable(&delta)
                .with_context(|| format!("@{}:{}", file!(), line!()))
                .unwrap();

            write_vec_f(&mut channel, &out)
                .with_context(|| format!("@{}:{}", file!(), line!()))
                .unwrap();
        });

        let mut rng = AesRng::new();
        let reader = BufReader::new(receiver.try_clone().unwrap());
        let writer = BufWriter::new(receiver);
        let mut channel = Channel::new(reader, writer);

        let mut vole_receiver = OtVoleReceiver::<F128b, 128, OtReceiver>::new();
        let (a_vec, c_vec) = vole_receiver
            .receive(&mut channel, &mut rng, vole_size)
            .with_context(|| format!("@{}:{}", file!(), line!()))
            .unwrap();

        let delta: F128b = channel.read_serializable().unwrap();

        let b_vec: Vec<F128b> = read_vec_f(&mut channel)
            .with_context(|| format!("@{}:{}", file!(), line!()))
            .unwrap();

        handle.join().unwrap();

        dbg!(b_vec.len());

        for ((a, b), c) in a_vec
            .into_iter()
            .zip(b_vec.into_iter())
            .zip(c_vec.into_iter())
        {
            assert_eq!(delta * a + b, c);
        }
    }

    #[test]
    fn test_vole_share_small() {
        test_vole_share_base(100);
    }

    #[test]
    fn test_vole_share_middle() {
        for e in 1..=17 {
            println!("n = 2^{} = {}", e, 2usize.pow(e));
            test_vole_share_base(2usize.pow(e));
        }
    }

    #[test]
    fn test_vole_share_large() {
        for e in 18..=20 {
            println!("n = 2^{} = {}", e, 2usize.pow(e));
            test_vole_share_base(2usize.pow(e));
        }
    }

    #[test]
    fn test_vole_share_20() {
        test_vole_share_base(2usize.pow(20));
    }
}
