use crate::channel_utils::{read_vec_f, write_vec_f};
use crate::preprocessed::oprf::{
    SepOprfReceiver, SepOprfReceiverWithVole, SepOprfSender, SepOprfSenderWithVole,
};
use crate::solver::Solver;
use crate::vole::{VoleShareForReceiver, VoleShareForSender};
use anyhow::{anyhow, Context, Error};
use rand::{CryptoRng, Rng};
use scuttlebutt::field::FiniteField as FF;
use scuttlebutt::AbstractChannel;
use std::clone::Clone;

pub trait ObliviousProgrammablePrf
where
    Self: Sized,
{
    type Seed;
    type Input;
    type Output;
}

pub trait SepOpprfSender: ObliviousProgrammablePrf
where
    Self: Sized,
{
    type PrecompSystem;

    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        system: Self::PrecompSystem,
    ) -> Result<Self, Error>;

    fn send<C, RNG>(
        self,
        channel: &mut C,
        points: &[(Self::Input, Self::Output)],
        query_num: usize,
        rng: &mut RNG,
    ) -> Result<Box<dyn Fn(Self::Input) -> Result<Self::Output, Error> + Send>, Error>
    where
        C: AbstractChannel,
        RNG: CryptoRng + Rng;

    // fn compute(&self, input: Self::Input) -> Result<Self::Output, Error>;
}

pub trait SepOpprfReceiver: ObliviousProgrammablePrf
where
    Self: Sized,
{
    type PrecompSystem;

    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        system: Self::PrecompSystem,
    ) -> Result<Self, Error>;

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

pub struct SepOpprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    params: S::Params,
    oprf_sender: SepOprfSenderWithVole<F, S, V>,
    // fk: Option<Box<dyn Fn(&Self, F) -> Result<F, Error> + Send>>,
}

impl<F, S, V> ObliviousProgrammablePrf for SepOpprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    type Seed = ();
    type Input = F;
    type Output = F;
}

impl<F, S, V> SepOpprfSender for SepOpprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    type PrecompSystem = V;

    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        system: Self::PrecompSystem,
    ) -> Result<Self, Error> {
        let params = S::calc_params(query_num);
        let oprf_sender = SepOprfSenderWithVole::precomp(channel, rng, query_num, system)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        Ok(Self {
            params,
            oprf_sender,
            // fk: None,
        })
    }

    fn send<C, RNG>(
        self,
        channel: &mut C,
        points: &[(Self::Input, Self::Output)],
        query_num: usize,
        rng: &mut RNG,
    ) -> Result<Box<dyn Fn(F) -> Result<F, Error> + Send>, Error>
    where
        C: AbstractChannel,
        RNG: CryptoRng + Rng,
    {
        let fk = self
            .oprf_sender
            .send(channel, query_num, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let points = points
            .iter()
            .map(|&(x, z)| {
                let y = z - (fk(x).with_context(|| format!("@{}:{}", file!(), line!()))?);
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

        write_vec_f(channel, &p).with_context(|| format!("@{}:{}", file!(), line!()))?;

        let params = self.params.clone();
        let fk = move |x: F| -> Result<F, Error> {
            let d = S::decode(&p, x, aux, params)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            let res = d + fk(x).with_context(|| format!("@{}:{}", file!(), line!()))?;
            Ok(res)
        };

        Ok(Box::new(fk))
    }

    /*
    fn compute(&self, input: Self::Input) -> Result<Self::Output, Error> {
        let fk = self.fk.as_ref().unwrap();
        let res = fk(self, input).with_context(|| format!("@{}:{}", file!(), line!()))?;
        Ok(res)
    }
    */
}

pub struct SepOpprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    params: S::Params,
    oprf_receiver: SepOprfReceiverWithVole<F, S, V>,
}

impl<F, S, V> ObliviousProgrammablePrf for SepOpprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    type Seed = ();
    type Input = F;
    type Output = F;
}

impl<F, S, V> SepOpprfReceiver for SepOpprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    type PrecompSystem = V;

    fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        rng: &mut RNG,
        query_num: usize,
        system: Self::PrecompSystem,
    ) -> Result<Self, Error> {
        let params = S::calc_params(query_num);
        let oprf_receiver = SepOprfReceiverWithVole::precomp(channel, rng, query_num, system)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        Ok(Self {
            params,
            oprf_receiver,
        })
    }

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
        let Self {
            params,
            oprf_receiver,
        } = self;

        let oprf_res = oprf_receiver
            .receive(channel, queries, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let aux =
            S::aux_receive(channel, rng).with_context(|| format!("@{}:{}", file!(), line!()))?;

        let p = read_vec_f(channel).with_context(|| format!("@{}:{}", file!(), line!()))?;

        let points = oprf_res
            .iter()
            .map(|&(x, fkx)| {
                let y = S::decode(&p, x, aux, params)
                    .with_context(|| format!("@{}:{}", file!(), line!()))?
                    + fkx;
                Ok((x, y))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(points)
    }
}

// You are allowed to clone them FOR BENCHMARKING PURPOSES ONLY.
// DO NOT USE THEM IN PRODUCTION because of the security reasons.

impl<F, S, V> Clone for SepOpprfSenderWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForSender<F>,
{
    fn clone(&self) -> Self {
        Self {
            params: self.params,
            oprf_sender: self.oprf_sender.clone(),
            // fk: None,
        }
    }
}

impl<F, S, V> Clone for SepOpprfReceiverWithVole<F, S, V>
where
    F: FF,
    S: Solver<F>,
    V: VoleShareForReceiver<F>,
{
    fn clone(&self) -> Self {
        Self {
            params: self.params,
            oprf_receiver: self.oprf_receiver.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::{PaxosSolver, Solver, SolverParams, VandelmondeSolver};
    use crate::vole::{
        LPNVoleReceiver, LPNVoleSender, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM,
        LPN_SETUP_SMALL,
    };
    use rand::distributions::{Distribution, Standard};
    use rand::seq::SliceRandom;
    use scuttlebutt::serialization::CanonicalSerialize;
    use scuttlebutt::{field::F128b, AesRng, Channel};
    use std::collections::{HashMap, HashSet};
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

    #[allow(non_snake_case)]
    fn usize2F128b(x: usize) -> F128b {
        let x = x as u128;
        let x = x.to_le_bytes();
        let res = F128b::from_bytes(&x.into()).unwrap();

        // dbg!(res);

        res
    }

    fn test_sep_opprf_base<S: Solver<F128b>>(set_size: usize, common_size: usize, verbose: bool) {
        let (sender_set, receiver_set, intersection) = create_sets::<F128b>(set_size, common_size);
        let points = sender_set
            .iter()
            .enumerate()
            .map(|(i, &x)| (x, usize2F128b(i)))
            .collect::<Vec<_>>();

        let sender_set_2 = sender_set.clone();
        let points_2 = points.clone();

        let points: HashMap<F128b, F128b> = HashMap::from_iter(points.into_iter());

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
        }

        let (sender, receiver) = UnixStream::pair().unwrap();
        let handle = std::thread::spawn(move || {
            let sender_set = sender_set_2;
            let points = points_2;

            let mut rng = AesRng::new();
            let reader = BufReader::new(sender.try_clone().unwrap());
            let writer = BufWriter::new(sender);
            let mut channel = Channel::new(reader, writer);

            let vole_share_for_s = LPNVoleSender::new(setup_params, extend_params);

            let opprf_sender = SepOpprfSenderWithVole::<F128b, S, _>::precomp(
                &mut channel,
                &mut rng,
                sender_set.len(),
                vole_share_for_s,
            )
            .unwrap();

            let fk = opprf_sender
                .send(&mut channel, &points, points.len(), &mut rng)
                .unwrap();

            for &(x, y) in points.iter() {
                let y_computed = fk(x).unwrap();
                assert_eq!(y, y_computed);
            }
        });

        let mut rng = AesRng::new();
        let reader = BufReader::new(receiver.try_clone().unwrap());
        let writer = BufWriter::new(receiver);
        let mut channel = Channel::new(reader, writer);

        let vole_share_for_r = LPNVoleReceiver::new(setup_params, extend_params);

        let opprf_receiver = SepOpprfReceiverWithVole::<F128b, S, _>::precomp(
            &mut channel,
            &mut rng,
            receiver_set.len(),
            vole_share_for_r,
        )
        .unwrap();

        let received = opprf_receiver
            .receive(&mut channel, &receiver_set, &mut rng)
            .unwrap();

        handle.join().unwrap();

        if verbose {
            dbg!(received.clone());
        }

        for (x, y) in received {
            if let Some(&original_y) = points.get(&x) {
                assert_eq!(y, original_y);
                if verbose {
                    println!("{:?} is in the sender set. and f({:?}) = {:?}", x, x, y);
                }
            } else if verbose {
                println!("{:?} is not in the sender set", x);
            }
        }
    }

    #[test]
    fn test_sep_opprf_vandelmonde_small() {
        test_sep_opprf_base::<VandelmondeSolver<F128b>>(10, 5, true);
    }

    #[test]
    fn test_sep_opprf_paxos_small() {
        test_sep_opprf_base::<PaxosSolver<F128b>>(10, 5, true);
    }

    #[test]
    fn test_sep_opprf_paxos_middle() {
        test_sep_opprf_base::<PaxosSolver<F128b>>(100, 50, false);
    }

    #[test]
    fn test_sep_opprf_paxos_large() {
        test_sep_opprf_base::<PaxosSolver<F128b>>(1 << 12, 1 << 6, false);
    }
}
