use crate::channel_utils::{read_vec_f, write_vec_f};
use crate::hash_utils::{hash, hash_f};
use crate::solver::{Solver, SolverParams};
use crate::vole::{VoleShareForReceiver, VoleShareForSender};
use anyhow::{anyhow, bail, Context, Error};
use ocelot::oprf::ObliviousPrf;
use rand::{CryptoRng, Rng};
use scuttlebutt::field::FiniteField as FF;
use scuttlebutt::AbstractChannel;
use std::clone::Clone;
use std::marker::PhantomData;

/// Trait indicating that OPRF constraints are satisfied.
pub trait SepOprfSender: ObliviousPrf
where
    Self: Sized,
{
    /// Precomputation system. e.g. [OtVoleSender](crate::vole::OtVoleSender), [LPNVoleSender](crate::vole::LPNVoleSender), etc. These system will implement [VoleShareForSender] trait in this library.
    type PrecompSystem;

    /// Precomputation for the sender. It runned in the offline phase.
    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        system: Self::PrecompSystem,
    ) -> Result<Self, Error>;

    /// Main protocol for the sender. It runned in the online phase.
    fn send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        self,
        channel: &mut C,
        query_num: usize,
        rng: &mut RNG,
    ) -> Result<Box<dyn Fn(Self::Input) -> Result<Self::Output, Error> + Send>, Error>;

    // fn compute(&self, input: Self::Input) -> Result<Self::Output, Error>;
}

/// Trait for Separated OPRF Receiver.
pub trait SepOprfReceiver: ObliviousPrf
where
    Self: Sized,
{
    /// Precomputation system. e.g. [OtVoleSender](crate::vole::OtVoleSender), [LPNVoleSender](crate::vole::LPNVoleSender), etc. These system will implement [VoleShareForReceiver] trait in this library.
    type PrecompSystem;

    /// Precomputation for the receiver. It runned in the offline phase.
    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        system: Self::PrecompSystem,
    ) -> Result<Self, Error>;

    /// Main protocol for the receiver. It runned in the online phase.
    fn receive<C, RNG>(
        self,
        channel: &mut C,
        queries: &[Self::Input],
        rng: &mut RNG,
    ) -> Result<Vec<(Self::Input, Self::Output)>, Error>
    where
        C: AbstractChannel,
        RNG: CryptoRng + Rng;
}

/// Actual implementation of Separated OPRF sender using VOLE.
pub struct SepOprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    params: S::Params,
    delta: F,
    vec_b: Vec<F>,
    // fk: Option<Box<dyn Fn(F) -> Result<F, Error> + Send>>,
    _p: PhantomData<(F, S, V)>,
}

impl<F, S, V> ObliviousPrf for SepOprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    type Seed = ();
    type Input = F;
    type Output = F;
}

impl<F, S, V> SepOprfSender for SepOprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    type PrecompSystem = V;

    /// Actual implementation of precomputation for the sender. It called in offline phase and VOLE sharing is run.
    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        mut vole_share_for_s: V,
    ) -> Result<Self, Error> {
        let params = S::calc_params(query_num);
        let m = params.code_length();

        let (delta, vec_b) = vole_share_for_s
            .receive(channel, rng, m)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        if vec_b.len() != m {
            bail!(
                "vec_b.len() (={}) != m (={}) @ {}:{}",
                vec_b.len(),
                m,
                file!(),
                line!()
            );
        }

        Ok(Self {
            params,
            delta,
            vec_b,
            // fk: None,
            _p: PhantomData,
        })
    }

    /// Actual implementation of send protocol. It called in online phase and solver decoding is run.
    fn send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        self,
        channel: &mut C,
        _query_num: usize,
        rng: &mut RNG,
    ) -> Result<Box<dyn Fn(F) -> Result<F, Error> + Send>, Error> {
        let aux =
            S::aux_receive(channel, rng).with_context(|| format!("@{}:{}", file!(), line!()))?;

        let a_dash: Vec<F> = read_vec_f(channel)?;

        let m = self.params.code_length();
        if a_dash.len() != m {
            bail!(
                "a_dash.len() (={}) != (={}) m @ {}:{}",
                a_dash.len(),
                m,
                file!(),
                line!()
            );
        }

        let delta = self.delta;

        let k = a_dash
            .into_iter()
            .zip(self.vec_b.iter())
            .map(|(ad, &b)| delta * ad + b)
            .collect::<Vec<_>>();

        let params = self.params.clone();
        let fk = move |x| -> Result<F, Error> {
            let d = S::decode(&k, x, aux, params)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            let f_dash =
                d - (delta * hash_f(x).with_context(|| format!("@{}:{}", file!(), line!()))?);
            let res = hash(f_dash, x).with_context(|| format!("@{}:{}", file!(), line!()))?;
            Ok(res)
        };

        Ok(Box::new(fk))
    }

    /*
    fn compute(&self, input: Self::Input) -> Result<Self::Output, Error> {
        let Some(fk) = &self.fk else {
            bail!("k has not been set yet. @ {}:{}", file!(), line!());
        };

        Ok(fk(input).with_context(|| format!("@{}:{}", file!(), line!()))?)
    }
    */
}

/// Actual implementation of Separated OPRF receiver using VOLE.
pub struct SepOprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    params: S::Params,
    vec_a: Vec<F>,
    vec_c: Vec<F>,
    _p: PhantomData<(F, S, V)>,
}

impl<F, S, V> ObliviousPrf for SepOprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    type Seed = ();
    type Input = F;
    type Output = F;
}

impl<F, S, V> SepOprfReceiver for SepOprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    type PrecompSystem = V;

    /// Actual implementation of precomputation for the receiver. It called in offline phase and VOLE sharing is run.
    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        mut vole_share_for_r: V,
    ) -> Result<Self, Error> {
        let params = S::calc_params(query_num);
        let m = params.code_length();

        let (vec_a, vec_c) = vole_share_for_r
            .receive(channel, rng, m)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        if vec_a.len() != m || vec_c.len() != m {
            bail!(
                "vec_a.len() (={}) != m (={}) or vec_c.len() (={}) != m @ {}:{}",
                vec_a.len(),
                m,
                vec_c.len(),
                file!(),
                line!()
            );
        }

        Ok(Self {
            params,
            vec_a,
            vec_c,
            _p: PhantomData,
        })
    }

    /// Actual implementation of receive protocol. It called in online phase and solver encoding (e.g. cukoo graph creating by PaXoS solver) is run.
    fn receive<C, RNG>(
        self,
        channel: &mut C,
        queries: &[Self::Input],
        rng: &mut RNG,
    ) -> Result<Vec<(Self::Input, Self::Output)>, Error>
    where
        C: AbstractChannel,
        RNG: CryptoRng + Rng,
    {
        let points = queries
            .iter()
            .map(|input| {
                let x = input.clone();
                let y = hash_f(x).with_context(|| format!("@{}:{}", file!(), line!()))?;
                Ok((x, y))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let mut aux = S::gen_aux(rng).with_context(|| format!("@{}:{}", file!(), line!()))?;
        let mut p = Err(anyhow!("dummy!"));
        for _ in 0..2 {
            p = S::encode(rng, &points, aux, self.params)
                .with_context(|| format!("@{}:{}", file!(), line!()));
            if p.is_ok() {
                break;
            }
            aux = S::gen_aux(rng).with_context(|| format!("@{}:{}", file!(), line!()))?;
        }
        let p = p?;

        S::aux_send(channel, rng, aux).with_context(|| format!("@{}:{}", file!(), line!()))?;

        if p.len() != self.vec_a.len() {
            bail!(
                "p.len() (={}) != vec_a.len() (={}) @ {}:{}",
                p.len(),
                self.vec_a.len(),
                file!(),
                line!()
            );
        }

        let p_plus_a = p
            .iter()
            .zip(self.vec_a.iter())
            .map(|(&p, &a)| p + a)
            .collect::<Vec<_>>();

        write_vec_f(channel, &p_plus_a).with_context(|| format!("@{}:{}", file!(), line!()))?;

        let res = queries
            .into_iter()
            .map(|&x| {
                let d = S::decode(&self.vec_c, x, aux, self.params)
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
                let y = hash(d, x).with_context(|| format!("@{}:{}", file!(), line!()))?;
                Ok((x, y))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(res)
    }
}

/// You are allowed to clone them **FOR BENCHMARKING PURPOSES ONLY**.
///
/// **DO NOT USE THEM IN PRODUCTION** because of the security reasons.
impl<F, S, V> Clone for SepOprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    fn clone(&self) -> Self {
        Self {
            params: self.params,
            delta: self.delta,
            vec_b: self.vec_b.clone(),
            // fk: None,
            _p: PhantomData,
        }
    }
}

/// You are allowed to clone them **FOR BENCHMARKING PURPOSES ONLY**.
///
/// **DO NOT USE THEM IN PRODUCTION** because of the security reasons.
impl<F, S, V> Clone for SepOprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    fn clone(&self) -> Self {
        Self {
            params: self.params,
            vec_a: self.vec_a.clone(),
            vec_c: self.vec_c.clone(),
            _p: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::{PaxosSolver, VandelmondeSolver};
    use crate::vole::{
        LPNVoleReceiver, LPNVoleSender, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM,
        LPN_SETUP_SMALL,
    };
    use rand::distributions::{Distribution, Standard};
    use rand::seq::SliceRandom;
    use scuttlebutt::{field::F128b, AesRng, Channel};
    use std::collections::HashSet;
    use std::io::{BufReader, BufWriter};
    use std::os::unix::net::UnixStream;

    fn create_sets<F: FF>(set_size: usize, common_size: usize) -> (Vec<F>, Vec<F>, Vec<F>)
    where
        Standard: Distribution<F>,
    {
        if set_size < common_size {
            panic!("set_size (={}) < common_size (={})", set_size, common_size);
        }

        let mut rng = AesRng::new();
        let common = (0..common_size).map(|_| rng.gen::<F>()).collect::<Vec<_>>();

        let mut set1 = HashSet::<F>::from_iter(common.clone().into_iter());
        while set1.len() < set_size {
            set1.insert(rng.gen::<F>());
        }

        let mut set2 = HashSet::<F>::from_iter(common.clone().into_iter());
        while set2.len() < set_size {
            set2.insert(rng.gen::<F>());
        }

        let common = set1
            .iter()
            .filter_map(|x| if set2.contains(x) { Some(*x) } else { None })
            .collect::<Vec<_>>();

        let mut set1 = set1.into_iter().collect::<Vec<F>>();
        set1.shuffle(&mut rng);
        let mut set2 = set2.into_iter().collect::<Vec<_>>();
        set2.shuffle(&mut rng);

        (set1, set2, common)
    }

    fn test_2party_psi_base<S: Solver<F128b>>(set_size: usize, common_size: usize, verbose: bool) {
        let (sender_set, receiver_set, intersection) = create_sets::<F128b>(set_size, common_size);

        let m_size = S::calc_params(set_size).code_length();
        let (setup_params, extend_params) = if m_size < (1 << 17) {
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };

        if verbose {
            println!("sender_set = {:?}\nlen: {}", sender_set, sender_set.len());
            println!(
                "receiver_set = {:?}\nlen: {}",
                receiver_set,
                receiver_set.len()
            );
            println!(
                "intersection = {:?}\nlen: {}",
                intersection,
                intersection.len()
            );
        } else {
            println!("set_size = {}", set_size);
            println!("common_size = {}", common_size);
        }

        let (sender, receiver) = UnixStream::pair().unwrap();
        let handle = std::thread::spawn(move || {
            let mut rng = AesRng::new();
            let reader = BufReader::new(sender.try_clone().unwrap());
            let writer = BufWriter::new(sender);
            let mut channel = Channel::new(reader, writer);

            let vole_share_for_s = LPNVoleSender::new(setup_params, extend_params);

            let oprf_sender = SepOprfSenderWithVole::<F128b, S, _>::precomp(
                &mut channel,
                &mut rng,
                sender_set.len(),
                vole_share_for_s,
            )
            .unwrap();

            println!("sender precomp done.");

            let fk = oprf_sender
                .send(&mut channel, sender_set.len(), &mut rng)
                .unwrap();

            println!("sender send done.");

            let mut fk_set = sender_set
                .iter()
                .map(|&x| fk(x).unwrap())
                .collect::<Vec<_>>();

            fk_set.shuffle(&mut rng);

            write_vec_f(&mut channel, &fk_set).unwrap();

            println!("sender write_vec_f done.");

            let _intersection: Vec<F128b> = read_vec_f(&mut channel).unwrap();

            println!("sender compute done.");
        });

        let mut rng = AesRng::new();
        let reader = BufReader::new(receiver.try_clone().unwrap());
        let writer = BufWriter::new(receiver);
        let mut channel = Channel::new(reader, writer);

        let vole_share_for_r = LPNVoleReceiver::new(setup_params, extend_params);

        let oprf_receiver = SepOprfReceiverWithVole::<F128b, S, _>::precomp(
            &mut channel,
            &mut rng,
            receiver_set.len(),
            vole_share_for_r,
        )
        .unwrap();

        println!("receiver precomp done.");

        let received = oprf_receiver
            .receive(&mut channel, &receiver_set, &mut rng)
            .unwrap();

        println!("receiver receive done.");

        if verbose {
            dbg!(received.clone());
        }

        let sender_fk_set: Vec<F128b> = read_vec_f(&mut channel).unwrap();

        println!("receiver read_vec_f done.");

        if verbose {
            dbg!(sender_fk_set.clone());
        }

        let res = received
            .into_iter()
            .filter_map(|(x, y)| {
                if sender_fk_set.contains(&y) {
                    Some(x)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        write_vec_f(&mut channel, &res).unwrap();

        println!("receiver compute done.");

        handle.join().unwrap();

        let res = HashSet::<F128b>::from_iter(res.into_iter());
        let intersection = HashSet::<F128b>::from_iter(intersection.into_iter());

        assert_eq!(res, intersection);
    }

    #[test]
    fn test_2party_psi_vandelmonde() {
        test_2party_psi_base::<VandelmondeSolver<F128b>>(10, 5, true);
    }

    #[test]
    fn test_2party_psi_paxos() {
        test_2party_psi_base::<PaxosSolver<F128b>>(10, 5, true);
    }

    #[test]
    fn test_2party_psi_paxos_middle() {
        test_2party_psi_base::<PaxosSolver<F128b>>(1 << 16, 1 << 15, false);
    }

    #[test]
    fn test_2party_psi_paxos_large() {
        test_2party_psi_base::<PaxosSolver<F128b>>(1 << 17, 1 << 16, false);
    }

    // If you want to finish below calculation within the expected time (60s), you should consider a more intelligent two-party PSI. there is no problem with PaXoS
    // The filter_map is taking a crazy amount of time.
    /*
    #[test]
    fn test_2party_psi_paxos_max() {
        test_2party_psi_base::<PaxosSolver<F128b>>(1 << 20, 1 << 16, false);
    }
    */
}
