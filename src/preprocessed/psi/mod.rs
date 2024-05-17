//! based on: <https://github.com/GaloisInc/swanky/blob/master/popsicle/src/psi/kmprt.rs>

use crate::preprocessed::opprf::{
    SepOpprfReceiver, SepOpprfReceiverWithVole, SepOpprfSender, SepOpprfSenderWithVole,
};
use crate::solver::Solver;
use crate::vole::{VoleShareForReceiver, VoleShareForSender};
use anyhow::{bail, Context, Error};
use rand::distributions::{Distribution, Standard};
use rand::{CryptoRng, Rng};
use scuttlebutt::channel::AbstractChannel;
use scuttlebutt::field::FiniteField as FF;
use std::clone::Clone;

mod bin;
mod multithread_ver;
pub use bin::run;

/// usize is used as a party ID. Receiver's ID is always 0.
pub type PartyId = usize;

struct Party<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    id: PartyId,
    opprf_senders: Vec<(usize, SepOpprfSenderWithVole<F, S, VS>)>,
    opprf_receivers: Vec<(usize, SepOpprfReceiverWithVole<F, S, VR>)>,
}

/// A kind of party in the protocol. They play sender and receiver in Conditional Zero Sharing, and play sender in Conditional Reconstruction.
///
/// `*_mt` means multi-threads optimization.
///
/// Not optimized version doesn't mean single-threaded version. The difference between the optimized version and the not one is that in where parties exchange messages.
///
/// In the optimized version, each party has a separate thread to communicate with each of the other parties.
///
/// On the other hand, in the not optimized version, each party communicates in the same thread with all the other parties.
pub struct Sender<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    id: PartyId,
    party_for_zs: Party<F, S, VS, VR>,
    opprf_sender_for_rc: SepOpprfSenderWithVole<F, S, VS>,
}

impl<F, S, VS, VR> Sender<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    /// Get the party ID. Receiver is always 0.
    pub fn get_id(&self) -> PartyId {
        self.id
    }

    /// Precomputation for the sender. It runned in the offline phase.
    pub fn precomp<C: AbstractChannel, RNG: Rng + CryptoRng>(
        me: PartyId,
        channels: &mut [(PartyId, C)],
        rng: &mut RNG,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
        set_size: usize,
    ) -> Result<Self, Error> {
        if me == 0 {
            bail!("sender index must not be 0. @{}:{}", file!(), line!());
        }

        let id = me;

        let party_for_zs = Party::precomp(
            me,
            channels,
            rng,
            vole_share_for_s,
            vole_share_for_r,
            set_size,
        )
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let opprf_sender_for_rc =
            SepOpprfSenderWithVole::precomp(&mut channels[0].1, rng, set_size, vole_share_for_s)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok(Self {
            id,
            party_for_zs,
            opprf_sender_for_rc,
        })
    }

    /// Send protocol which consists of conditional secret sharing and conditional reconstruction sending.
    /// It runned in the online phase.
    pub fn send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        self,
        inputs: &[F],
        channels: &mut [(PartyId, C)],
        rng: &mut RNG,
    ) -> Result<(), Error> {
        assert!(self.id != 0);

        let Self {
            id: _,
            party_for_zs,
            opprf_sender_for_rc,
        } = self;

        // conditional zero sharing
        let s_hat_sum = party_for_zs.conditional_secret_sharing(inputs, channels, rng)?;

        // conditional reconstruction
        let points = inputs
            .iter()
            .cloned()
            .zip(s_hat_sum.into_iter())
            .collect::<Vec<_>>();
        let channel = &mut channels[0].1;
        let _fk = opprf_sender_for_rc
            .send(channel, &points, inputs.len(), rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok(())
    }
}

/// A kind of party in the protocol. They play sender and receiver in Conditional Zero Sharing, and play receiver in Conditional Reconstruction.
///
/// `*_mt` means multi-threads optimization.
///
/// Not optimized version doesn't mean single-threaded version. The difference between the optimized version and the not one is that in where parties exchange messages.
///
/// In the optimized version, each party has a separate thread to communicate with each of the other parties.
///
/// On the other hand, in the not optimized version, each party communicates in the same thread with all the other parties.
pub struct Receiver<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    party_for_zs: Party<F, S, VS, VR>,
    opprf_receivers_for_rc: Vec<(usize, SepOpprfReceiverWithVole<F, S, VR>)>,
}

impl<F, S, VS, VR> Receiver<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    /// Get the party ID. Receiver is always 0.
    pub fn get_id(&self) -> PartyId {
        0
    }

    /// Precomputation for the receiver. It runned in the offline phase.
    pub fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channels: &mut [(PartyId, C)],
        rng: &mut RNG,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
        set_size: usize,
    ) -> Result<Self, Error> {
        let party_for_zs = Party::precomp(
            0,
            channels,
            rng,
            vole_share_for_s,
            vole_share_for_r,
            set_size,
        )
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let opprf_receivers_for_rc = channels
            .iter_mut()
            .map(|(them, channel)| {
                let rcvr =
                    SepOpprfReceiverWithVole::precomp(channel, rng, set_size, vole_share_for_r)
                        .with_context(|| format!("@{}:{}", file!(), line!()))?;
                Ok((*them, rcvr))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(Self {
            party_for_zs,
            opprf_receivers_for_rc,
        })
    }

    /// Receive protocol which consists of conditional secret sharing and conditional reconstruction receiving.
    /// It runned in the online phase.
    pub fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        self,
        inputs: &[F],
        channels: &mut [(PartyId, C)],
        rng: &mut RNG,
    ) -> Result<Vec<F>, Error> {
        let Self {
            party_for_zs,
            opprf_receivers_for_rc,
        } = self;

        // conditional zero sharing
        let mut s_hat_sum = party_for_zs.conditional_secret_sharing(inputs, channels, rng)?;

        // conditional reconstruction
        for ((them, channel), (ri, receiver)) in
            channels.iter_mut().zip(opprf_receivers_for_rc.into_iter())
        {
            assert!(ri == *them);

            let shares = receiver
                .receive(channel, inputs, rng)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            for (i, (_, y)) in shares.into_iter().enumerate() {
                s_hat_sum[i] += y;
            }
        }

        let intersection = inputs
            .iter()
            .zip(s_hat_sum.into_iter())
            .filter_map(|(&x, s)| if s.is_zero() { Some(x) } else { None })
            .collect::<Vec<_>>();

        Ok(intersection)
    }
}

impl<F, S, VS, VR> Party<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    pub fn precomp<C: AbstractChannel, RNG: CryptoRng + Rng>(
        me: PartyId,
        channels: &mut [(PartyId, C)],
        rng: &mut RNG,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
        set_size: usize,
    ) -> Result<Self, Error> {
        let mut opprf_senders = Vec::with_capacity(channels.len());
        let mut opprf_receivers = Vec::with_capacity(channels.len());

        for (them, channel) in channels.iter_mut() {
            // the party with the lowest PID gets to initialize their OPPRF sender first
            if me < *them {
                let sndr =
                    SepOpprfSenderWithVole::precomp(channel, rng, set_size, vole_share_for_s)
                        .with_context(|| format!("@{}:{}", file!(), line!()))?;
                opprf_senders.push((*them, sndr));

                let rcvr =
                    SepOpprfReceiverWithVole::precomp(channel, rng, set_size, vole_share_for_r)
                        .with_context(|| format!("@{}:{}", file!(), line!()))?;
                opprf_receivers.push((*them, rcvr));
            } else {
                let rcvr =
                    SepOpprfReceiverWithVole::precomp(channel, rng, set_size, vole_share_for_r)
                        .with_context(|| format!("@{}:{}", file!(), line!()))?;
                opprf_receivers.push((*them, rcvr));

                let sndr =
                    SepOpprfSenderWithVole::precomp(channel, rng, set_size, vole_share_for_s)
                        .with_context(|| format!("@{}:{}", file!(), line!()))?;
                opprf_senders.push((*them, sndr));
            }
        }

        Ok(Self {
            id: me,
            opprf_senders,
            opprf_receivers,
        })
    }

    fn conditional_secret_sharing<C: AbstractChannel, RNG: CryptoRng + Rng>(
        self,
        inputs: &[F],
        channels: &mut [(PartyId, C)],
        rng: &mut RNG,
    ) -> Result<Vec<F>, Error> {
        let nparties = channels.len() + 1;
        let ninputs = inputs.len();

        // s_hat_sum[k]: k th item's share sum for me.
        let mut s_hat_sum = vec![F::zero(); ninputs];

        // s[k][i]: k th item's share for P_i.
        let s = (0..ninputs)
            .map(|k| {
                let shares = secret_sharing_of_zero(nparties, rng);
                s_hat_sum[k] = shares[self.id];
                shares
            })
            .collect::<Vec<Vec<F>>>();

        let Self {
            id: _,
            opprf_senders,
            opprf_receivers,
        } = self;

        for (((other_id, channel), (si, sender)), (ri, receiver)) in channels
            .iter_mut()
            .zip(opprf_senders.into_iter())
            .zip(opprf_receivers.into_iter())
        {
            let other_id = *other_id;
            assert!(other_id == si);
            assert!(other_id == ri);

            let points = inputs
                .iter()
                .enumerate()
                .map(|(k, &x)| (x, s[k][other_id]))
                .collect::<Vec<_>>();

            let s_hats;
            if self.id < other_id {
                let _fk = sender
                    .send(channel, &points, inputs.len(), rng)
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
                s_hats = receiver
                    .receive(channel, inputs, rng)
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
            } else {
                s_hats = receiver
                    .receive(channel, inputs, rng)
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
                let _fk = sender
                    .send(channel, &points, inputs.len(), rng)
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
            };

            for (k, &(_, s_hats)) in s_hats.iter().enumerate() {
                s_hat_sum[k] += s_hats;
            }
        }

        Ok(s_hat_sum)
    }
}

fn secret_sharing_of_zero<F: FF, R: Rng>(nparties: usize, rng: &mut R) -> Vec<F>
where
    Standard: Distribution<F>,
{
    let mut sum = F::zero();
    let mut shares = (0..nparties - 1)
        .map(|_| {
            let f = rng.gen();
            sum += f;
            f
        })
        .collect::<Vec<_>>();

    shares.push(sum);
    shares
}

/// You are allowed to clone them **FOR BENCHMARKING PURPOSES ONLY**.
///
/// **DO NOT USE THEM IN PRODUCTION** because of the security reasons.
impl<F, S, VS, VR> Clone for Party<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            opprf_senders: self.opprf_senders.clone(),
            opprf_receivers: self.opprf_receivers.clone(),
        }
    }
}

/// You are allowed to clone them **FOR BENCHMARKING PURPOSES ONLY**.
///
/// **DO NOT USE THEM IN PRODUCTION** because of the security reasons.
impl<F, S, VS, VR> Clone for Sender<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            party_for_zs: self.party_for_zs.clone(),
            opprf_sender_for_rc: self.opprf_sender_for_rc.clone(),
        }
    }
}

/// You are allowed to clone them **FOR BENCHMARKING PURPOSES ONLY**.
///
/// **DO NOT USE THEM IN PRODUCTION** because of the security reasons.
impl<F, S, VS, VR> Clone for Receiver<F, S, VS, VR>
where
    F: FF,
    S: Solver<F>,
    VS: VoleShareForSender<F>,
    VR: VoleShareForReceiver<F>,
    Standard: Distribution<F>,
{
    fn clone(&self) -> Self {
        Self {
            party_for_zs: self.party_for_zs.clone(),
            opprf_receivers_for_rc: self.opprf_receivers_for_rc.clone(),
        }
    }
}

// Japanese note (日本語でのメモ): ↑本当はbenchmark featureフラグ等を用意した方がよいが、煩雑になるため晒している

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel_utils::sync_channel::create_unix_channels;
    use crate::set_utils::create_sets_without_check;
    use crate::solver::{PaxosSolver, Solver, SolverParams, VandelmondeSolver};
    use crate::vole::{
        LPNVoleReceiver, LPNVoleSender, OtVoleReceiver, OtVoleSender, VoleShareForReceiver,
        VoleShareForSender, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM, LPN_SETUP_SMALL,
    };
    use num_traits::Zero;
    use ocelot::ot::{AlszReceiver as OtReceiver, AlszSender as OtSender};
    use rand::Rng;
    use scuttlebutt::field::F128b;
    use scuttlebutt::AesRng;
    use std::collections::HashSet;

    #[test]
    fn test_secret_sharing_of_zero() {
        let mut rng = AesRng::new();
        let nparties = (rng.gen::<usize>() % 98) + 2;
        let shares: Vec<F128b> = secret_sharing_of_zero(nparties, &mut rng);
        assert!(shares.len() == nparties);
        let mut sum = F128b::zero();
        for s in shares.into_iter() {
            assert!(s != F128b::zero());
            sum += s;
        }
        assert_eq!(sum, F128b::zero());
    }

    fn create_lpn_vole_sr<S: Solver<F128b>>(
        set_size: usize,
    ) -> (LPNVoleSender<F128b>, LPNVoleReceiver<F128b>) {
        let m_size = S::calc_params(set_size).code_length();
        let (setup_param, extend_param) = if m_size < (1 << 17) {
            println!("Small parameters are used.");
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            println!("Medium parameters are used.");
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };
        (
            LPNVoleSender::new(setup_param, extend_param),
            LPNVoleReceiver::new(setup_param, extend_param),
        )
    }

    fn test_protocol_base<S, VS, VR>(
        nparties: usize,
        set_size: usize,
        common_size: usize,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
    ) where
        S: Solver<F128b>,
        VS: VoleShareForSender<F128b> + 'static + Send,
        VR: VoleShareForReceiver<F128b> + 'static + Send,
    {
        let mut rng = AesRng::new();

        let (intersection, mut sets): (Vec<F128b>, Vec<Vec<F128b>>) =
            create_sets_without_check(nparties, set_size, common_size, &mut rng).unwrap();

        println!("intersection prepared.");

        // create channels
        let (mut receiver_channels, channels) = create_unix_channels(nparties).unwrap();

        for (i, mut channels) in channels.into_iter().enumerate() {
            // create and fork senders
            let pid = i + 1;
            let set = sets.pop().unwrap();
            let vole_share_for_s = vole_share_for_s.clone();
            let vole_share_for_r = vole_share_for_r.clone();
            std::thread::spawn(move || {
                let mut rng = AesRng::new();

                // offline phase
                let sender = Sender::<F128b, S, _, _>::precomp(
                    pid,
                    &mut channels,
                    &mut rng,
                    vole_share_for_s,
                    vole_share_for_r,
                    set_size,
                )
                .unwrap();

                println!("sender {} prepared.", pid);

                // online phase
                sender.send(&set, &mut channels, &mut rng).unwrap();

                println!("sender {} finished.", pid);
            });
        }

        // create and run receiver
        // offline phase
        // let vole_share_for_s = vole_share_for_s.clone();
        // let vole_share_for_r = vole_share_for_r.clone();
        let receiver = Receiver::<F128b, S, _, _>::precomp(
            &mut receiver_channels,
            &mut rng,
            vole_share_for_s,
            vole_share_for_r,
            set_size,
        )
        .unwrap();

        println!("receiver prepared.");

        // online phase
        let set = sets.pop().unwrap();
        let res = receiver
            .receive(&set, &mut receiver_channels, &mut rng)
            .unwrap();

        println!("receiver finished.");

        let res: HashSet<F128b> = HashSet::from_iter(res);
        let intersection: HashSet<F128b> = HashSet::from_iter(intersection);

        assert_eq!(res, intersection);
    }

    #[test]
    fn test_protocol_vandelmonde_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        let (vole_share_for_s, vole_share_for_r) =
            create_lpn_vole_sr::<VandelmondeSolver<F128b>>(set_size);
        test_protocol_base::<VandelmondeSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }

    #[test]
    fn test_protocol_paxos_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        let (vole_share_for_s, vole_share_for_r) =
            create_lpn_vole_sr::<PaxosSolver<F128b>>(set_size);
        test_protocol_base::<PaxosSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }

    #[test]
    fn test_protocol_paxos_middle() {
        let nparties = 5;
        let set_size = 1 << 10;
        let common_size = 1 << 5;
        let (vole_share_for_s, vole_share_for_r) =
            create_lpn_vole_sr::<PaxosSolver<F128b>>(set_size);
        test_protocol_base::<PaxosSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }

    #[test]
    fn test_protocol_paxos_large() {
        let nparties = 5;
        let set_size = 1 << 20;
        let common_size = 1 << 5;
        let (vole_share_for_s, vole_share_for_r) =
            create_lpn_vole_sr::<PaxosSolver<F128b>>(set_size);
        test_protocol_base::<PaxosSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }

    #[test]
    fn test_protocol_paxos_small_with_ot() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        let vole_share_for_s = OtVoleSender::<F128b, 128, OtSender>::new();
        let vole_share_for_r = OtVoleReceiver::<F128b, 128, OtReceiver>::new();
        test_protocol_base::<PaxosSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }

    #[test]
    fn test_protocol_paxos_middle_with_ot() {
        let nparties = 5;
        let set_size = 1 << 10;
        let common_size = 1 << 5;
        let vole_share_for_s = OtVoleSender::<F128b, 128, OtSender>::new();
        let vole_share_for_r = OtVoleReceiver::<F128b, 128, OtReceiver>::new();
        test_protocol_base::<PaxosSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }

    #[test]
    fn test_protocol_paxos_large_with_ot() {
        let nparties = 5;
        let set_size = 1 << 20;
        let common_size = 1 << 5;
        let vole_share_for_s = OtVoleSender::<F128b, 128, OtSender>::new();
        let vole_share_for_r = OtVoleReceiver::<F128b, 128, OtReceiver>::new();
        test_protocol_base::<PaxosSolver<F128b>, _, _>(
            nparties,
            set_size,
            common_size,
            vole_share_for_s,
            vole_share_for_r,
        );
    }
}
