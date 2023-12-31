use anyhow::{bail, Context, Error};
use ocelot::svole::wykw::Receiver as SVoleReceiverStruct;
use ocelot::svole::wykw::Sender as SVoleSenderStruct;
use ocelot::svole::SVoleReceiver as _;
use ocelot::svole::SVoleSender as _;
use rand::{CryptoRng, Rng};
use scuttlebutt::channel::AbstractChannel;
use scuttlebutt::field::FiniteField as FF;
use std::marker::PhantomData;

pub use ocelot::svole::wykw::{
    LpnParams, LPN_EXTEND_LARGE, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_LARGE,
    LPN_SETUP_MEDIUM, LPN_SETUP_SMALL,
};

pub trait VoleShareForSender<F: FF>: Clone + Copy {
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(F, Vec<F>), Error>;
}

pub trait VoleShareForReceiver<F: FF>: Clone + Copy {
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(Vec<F>, Vec<F>), Error>;
}

#[derive(Clone, Copy)]
pub struct LPNVoleSender<F: FF> {
    setup_param: LpnParams,
    extend_param: LpnParams,
    _ff: PhantomData<F>,
}

impl<F: FF> LPNVoleSender<F> {
    pub fn new(setup_param: LpnParams, extend_param: LpnParams) -> Self {
        Self {
            setup_param,
            extend_param,
            _ff: PhantomData,
        }
    }
}

impl<F: FF> VoleShareForSender<F> for LPNVoleSender<F> {
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(F, Vec<F>), Error> {
        let setup_param = self.setup_param;
        let extend_param = self.extend_param;
        let mut vole = SVoleReceiverStruct::init(channel, rng, setup_param, extend_param)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        let mut out = Vec::new();
        vole.receive(channel, rng, &mut out)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        if out.len() < m {
            bail!(
                "TOO BIG M!\nout.len() (={}) < m (={}) @ {}:{}",
                out.len(),
                m,
                file!(),
                line!()
            );
        }

        let vec_b = out[..m].to_vec();

        Ok((vole.delta(), vec_b))
    }
}

#[derive(Clone, Copy)]
pub struct LPNVoleReceiver<F: FF> {
    setup_param: LpnParams,
    extend_param: LpnParams,
    _ff: PhantomData<F>,
}

impl<F: FF> LPNVoleReceiver<F> {
    pub fn new(setup_param: LpnParams, extend_param: LpnParams) -> Self {
        Self {
            setup_param,
            extend_param,
            _ff: PhantomData,
        }
    }
}

impl<F: FF> VoleShareForReceiver<F> for LPNVoleReceiver<F> {
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(Vec<F>, Vec<F>), Error> {
        let setup_param = self.setup_param;
        let extend_param = self.extend_param;
        let mut vole = SVoleSenderStruct::init(channel, rng, setup_param, extend_param)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        let mut out: Vec<(F::PrimeField, F)> = Vec::new();
        vole.send(channel, rng, &mut out)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let mut a_vec = Vec::with_capacity(out.len());
        let mut c_vec = Vec::with_capacity(out.len());

        for (a, c) in out {
            a_vec.push(a.into());
            c_vec.push(c);
        }

        if a_vec.len() < m || c_vec.len() < m {
            bail!(
                "TOO BIG M!\nvec.len() (={} or {}) < m (={}) @ {}:{}",
                a_vec.len(),
                c_vec.len(),
                m,
                file!(),
                line!()
            );
        }

        let a_vec = a_vec[..m].to_vec();
        let c_vec = c_vec[..m].to_vec();

        Ok((a_vec, c_vec))
    }
}

#[cfg(test)]
mod tests {
    use crate::channel_utils::{read_vec_f, write_vec_f};

    use super::*;
    use scuttlebutt::{field::F128b, AesRng, Channel};
    use std::io::{BufReader, BufWriter};
    use std::os::unix::net::UnixStream;

    fn test_vole_share_base(vole_size: usize, setup_param: LpnParams, extend_param: LpnParams) {
        let (sender, receiver) = UnixStream::pair().unwrap();
        let handle = std::thread::spawn(move || {
            let mut rng = AesRng::new();
            let reader = BufReader::new(sender.try_clone().unwrap());
            let writer = BufWriter::new(sender);
            let mut channel = Channel::new(reader, writer);

            let mut vole_sender = LPNVoleSender::<F128b>::new(setup_param, extend_param);
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

        let mut vole_receiver = LPNVoleReceiver::<F128b>::new(setup_param, extend_param);
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
        let setup_param = LPN_SETUP_SMALL;
        let extend_param = LPN_EXTEND_SMALL;
        test_vole_share_base(100, setup_param, extend_param);
    }

    #[test]
    fn test_vole_share_middle() {
        for e in 1..=17 {
            println!("n = 2^{} = {}", e, 2usize.pow(e));
            let setup_param = LPN_SETUP_SMALL;
            let extend_param = LPN_EXTEND_SMALL;
            test_vole_share_base(2usize.pow(e), setup_param, extend_param);
        }
    }

    #[test]
    fn test_vole_share_large() {
        for e in 18..=20 {
            println!("n = 2^{} = {}", e, 2usize.pow(e));
            let setup_param = LPN_SETUP_MEDIUM;
            let extend_param = LPN_EXTEND_MEDIUM;
            test_vole_share_base(2usize.pow(e), setup_param, extend_param);
        }
    }

    use rand::distributions::{Distribution, Standard};
    use rand::Rng;
    use scuttlebutt::field::FiniteField;

    fn create_set<F: FiniteField>(set_size: usize) -> Vec<F>
    where
        Standard: Distribution<F>,
    {
        let mut rng = AesRng::new();

        let set = (0..set_size).map(|_| rng.gen()).collect::<Vec<_>>();

        set
    }

    #[test]
    fn test_vole_compute() {
        use crate::hash_utils::hash_f;
        use crate::solver::{Solver, VandelmondeSolver};

        const VOLE_SIZE: usize = 100;

        let set: Vec<F128b> = create_set(VOLE_SIZE);
        let set2 = set.clone();

        let (sender, receiver) = UnixStream::pair().unwrap();
        let handle = std::thread::spawn(move || {
            let set = set2;

            let mut rng = AesRng::new();
            let reader = BufReader::new(sender.try_clone().unwrap());
            let writer = BufWriter::new(sender);
            let mut channel = Channel::new(reader, writer);

            let setup_param = LPN_SETUP_SMALL;
            let extend_param = LPN_EXTEND_SMALL;

            let mut vole_sender = LPNVoleSender::<F128b>::new(setup_param, extend_param);
            let (delta, b_vec) = vole_sender
                .receive(&mut channel, &mut rng, VOLE_SIZE)
                .with_context(|| format!("@{}:{}", file!(), line!()))
                .unwrap();

            let a_dash_vec: Vec<F128b> = read_vec_f(&mut channel)
                .with_context(|| format!("@{}:{}", file!(), line!()))
                .unwrap();

            // k = b + delta * (a_dash)
            let k_vec = b_vec
                .iter()
                .zip(a_dash_vec.iter())
                .map(|(&b, &a_dash)| b + delta * a_dash)
                .collect::<Vec<_>>();

            let params = VandelmondeSolver::<F128b>::calc_params(set.len());

            let decoded = set
                .into_iter()
                .map(|x| (x, VandelmondeSolver::decode(&k_vec, x, (), params).unwrap()))
                .collect::<Vec<_>>();
            let res = decoded
                .into_iter()
                .map(|(x, y)| y - (delta * hash_f(x).unwrap()))
                .collect::<Vec<_>>();

            write_vec_f(&mut channel, &res).unwrap();
        });

        let mut rng = AesRng::new();
        let reader = BufReader::new(receiver.try_clone().unwrap());
        let writer = BufWriter::new(receiver);
        let mut channel = Channel::new(reader, writer);

        let setup_param = LPN_SETUP_SMALL;
        let extend_param = LPN_EXTEND_SMALL;

        let mut vole_receiver = LPNVoleReceiver::<F128b>::new(setup_param, extend_param);
        let (a_vec, c_vec) = vole_receiver
            .receive(&mut channel, &mut rng, VOLE_SIZE)
            .with_context(|| format!("@{}:{}", file!(), line!()))
            .unwrap();

        let params = VandelmondeSolver::<F128b>::calc_params(set.len());

        let points = set
            .iter()
            .map(|&x| (x, hash_f(x).unwrap()))
            .collect::<Vec<_>>();
        let p = VandelmondeSolver::encode(&mut rng, &points, (), params).unwrap();

        // a_dash = a + p
        let a_dash = a_vec
            .iter()
            .zip(p.iter())
            .map(|(&a, &p)| a + p)
            .collect::<Vec<_>>();

        write_vec_f(&mut channel, &a_dash).unwrap();

        let sender_res: Vec<F128b> = read_vec_f(&mut channel)
            .with_context(|| format!("@{}:{}", file!(), line!()))
            .unwrap();

        let res = set
            .into_iter()
            .map(|x| VandelmondeSolver::decode(&c_vec, x, (), params).unwrap())
            .collect::<Vec<_>>();

        handle.join().unwrap();

        assert_eq!(res, sender_res);
    }
}
