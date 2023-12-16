use super::{secret_sharing_of_zero, Party, PartyId, Receiver, Sender};
use crate::preprocessed::opprf::{
    SepOpprfReceiver, SepOpprfReceiverWithVole, SepOpprfSender, SepOpprfSenderWithVole,
};
use crate::solver::Solver;
use crate::vole::{VoleShareForReceiver, VoleShareForSender};
use anyhow::{bail, Context, Result};
use rand::distributions::{Distribution, Standard};
use scuttlebutt::channel::AbstractChannel;
use scuttlebutt::field::FiniteField as FF;
use scuttlebutt::AesRng;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

// *_mt means multi-threads

impl<F, S, VS, VR> Sender<F, S, VS, VR>
where
    F: FF,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    pub fn precomp_mt<C>(
        me: PartyId,
        channels: &mut [(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
        set_size: usize,
    ) -> Result<Self>
    where
        C: AbstractChannel + Sync + Send + 'static,
    {
        if me == 0 {
            bail!("sender index must not be 0. @{}:{}", file!(), line!());
        }

        let id = me;

        let party_for_zs = Party::precomp_mt(
            me,
            channels,
            rng,
            vole_share_for_s,
            vole_share_for_r,
            set_size,
        )
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let mut ch = channels[0].1.lock().unwrap();
        let channel: &mut C = &mut ch;
        let opprf_sender_for_rc =
            SepOpprfSenderWithVole::precomp(channel, rng, set_size, vole_share_for_s)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok(Self {
            id,
            party_for_zs,
            opprf_sender_for_rc,
        })
    }

    pub fn send_mt<C>(
        self,
        inputs: Arc<Vec<F>>,
        channels: &[(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<()>
    where
        C: AbstractChannel + Sync + Send + 'static,
    {
        assert!(self.id != 0);

        let Self {
            id: _,
            party_for_zs,
            opprf_sender_for_rc,
        } = self;

        // conditional zero sharing
        let inpts = Arc::clone(&inputs);
        let s_hat_sum = party_for_zs
            .conditional_secret_sharing_mt(inpts, channels, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        // conditional reconstruction
        let points = inputs
            .iter()
            .cloned()
            .zip(s_hat_sum.into_iter())
            .collect::<Vec<_>>();

        let mut ch = channels[0].1.lock().unwrap();
        let channel: &mut C = &mut ch;
        let _fk = opprf_sender_for_rc
            .send(channel, &points, inputs.len(), rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok(())
    }
}

impl<F, S, VS, VR> Receiver<F, S, VS, VR>
where
    F: FF,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    pub fn precomp_mt<C>(
        channels: &mut [(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
        set_size: usize,
    ) -> Result<Self>
    where
        C: AbstractChannel + Sync + Send + 'static,
    {
        let party_for_zs = Party::precomp_mt(
            0,
            channels,
            rng,
            vole_share_for_s,
            vole_share_for_r,
            set_size,
        )
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

        // channels.sort_by_key(|(them, _)| *them); // already sorted here.

        let (receiver_tx, receiver_rx) = channel();

        for (them, channel) in channels.iter() {
            let them = *them;
            let ch = Arc::clone(channel);
            let r_tx = receiver_tx.clone();
            let mut rng = rng.fork();

            std::thread::spawn(move || {
                let mut ch = ch.lock().unwrap();
                let channel: &mut C = &mut ch;
                let rcvr = SepOpprfReceiverWithVole::precomp(
                    channel,
                    &mut rng,
                    set_size,
                    vole_share_for_r,
                )
                .with_context(|| format!("@{}:{}", file!(), line!()));
                r_tx.send((them, rcvr)).unwrap();
            });
        }

        let mut opprf_receivers_for_rc = (0..channels.len())
            .map(|_| {
                let (i, r) = receiver_rx.recv().unwrap();
                Ok((i, r?))
            })
            .collect::<Result<Vec<_>>>()?;

        opprf_receivers_for_rc.sort_by_key(|(them, _)| *them);

        Ok(Self {
            party_for_zs,
            opprf_receivers_for_rc,
        })
    }

    pub fn receive_mt<C>(
        self,
        inputs: Arc<Vec<F>>,
        channels: &[(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<Vec<F>>
    where
        C: AbstractChannel + Sync + Send + 'static,
    {
        let Self {
            party_for_zs,
            opprf_receivers_for_rc,
        } = self;

        // conditional zero sharing
        let inpts = Arc::clone(&inputs);
        let mut s_hat_sum = party_for_zs
            .conditional_secret_sharing_mt(inpts, channels, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let (share_tx, share_rx) = channel();

        // conditional reconstruction
        for ((them, channel), (ri, receiver)) in
            channels.iter().zip(opprf_receivers_for_rc.into_iter())
        {
            assert!(ri == *them);
            let ch = Arc::clone(channel);
            let s_tx = share_tx.clone();
            let mut rng = rng.fork();
            let inputs = Arc::clone(&inputs);

            std::thread::spawn(move || {
                let mut ch = ch.lock().unwrap();
                let channel: &mut C = &mut ch;
                let shares = receiver
                    .receive(channel, &inputs, &mut rng)
                    .with_context(|| format!("@{}:{}", file!(), line!()));
                s_tx.send(shares).unwrap();
            });
        }

        for shares in share_rx.iter().take(channels.len()) {
            let shares = shares?;
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
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    pub fn precomp_mt<C>(
        me: PartyId,
        channels: &mut [(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
        vole_share_for_s: VS,
        vole_share_for_r: VR,
        set_size: usize,
    ) -> Result<Self>
    where
        C: AbstractChannel + Sync + Send + 'static,
    {
        let (sender_tx, sender_rx) = channel();
        let (receiver_tx, receiver_rx) = channel();

        channels.sort_by_key(|(them, _)| *them);

        for (them, channel) in channels.iter_mut() {
            let mut trng = rng.fork();
            let ch = Arc::clone(channel);
            let s_tx = sender_tx.clone();
            let r_tx = receiver_tx.clone();

            // the party with the lowest PID gets to initialize their OPPRF sender first
            let them = *them;
            if me < them {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let sndr = SepOpprfSenderWithVole::precomp(
                        channel,
                        &mut trng,
                        set_size,
                        vole_share_for_s,
                    )
                    .with_context(|| format!("@{}:{}", file!(), line!()));
                    s_tx.send((them, sndr)).unwrap();
                    let rcvr = SepOpprfReceiverWithVole::precomp(
                        channel,
                        &mut trng,
                        set_size,
                        vole_share_for_r,
                    )
                    .with_context(|| format!("@{}:{}", file!(), line!()));
                    r_tx.send((them, rcvr)).unwrap();
                });
            } else {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let rcvr = SepOpprfReceiverWithVole::precomp(
                        channel,
                        &mut trng,
                        set_size,
                        vole_share_for_r,
                    )
                    .with_context(|| format!("@{}:{}", file!(), line!()));
                    r_tx.send((them, rcvr)).unwrap();
                    let sndr = SepOpprfSenderWithVole::precomp(
                        channel,
                        &mut trng,
                        set_size,
                        vole_share_for_s,
                    )
                    .with_context(|| format!("@{}:{}", file!(), line!()));
                    s_tx.send((them, sndr)).unwrap();
                });
            }
        }

        let mut opprf_senders = (0..channels.len())
            .map(|_| {
                let (i, s) = sender_rx.recv().unwrap();
                Ok((i, s?))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut opprf_receivers = (0..channels.len())
            .map(|_| {
                let (i, r) = receiver_rx.recv().unwrap();
                Ok((i, r?))
            })
            .collect::<Result<Vec<_>>>()?;

        opprf_senders.sort_by_key(|(them, _)| *them);
        opprf_receivers.sort_by_key(|(them, _)| *them);

        Ok(Self {
            id: me,
            opprf_senders,
            opprf_receivers,
        })
    }

    fn conditional_secret_sharing_mt<C>(
        self,
        inputs: Arc<Vec<F>>,
        channels: &[(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<Vec<F>>
    where
        C: AbstractChannel + Sync + Send + 'static,
    {
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

        let (s_hats_tx, s_hats_rx) = channel();

        let Self {
            id: _,
            opprf_senders,
            opprf_receivers,
        } = self;

        for (((other_id, channel), (si, sender)), (ri, receiver)) in channels
            .iter()
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
            let mut trng = rng.fork();
            let ch = Arc::clone(channel);
            let sh_tx = s_hats_tx.clone();
            let inputs = Arc::clone(&inputs);

            if self.id < other_id {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let s_hats_w: Result<Vec<(F, F)>> = (|| {
                        let _fk = sender
                            .send(channel, &points, inputs.len(), &mut trng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        let s_hats = receiver
                            .receive(channel, &inputs, &mut trng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        Ok(s_hats)
                    })();
                    sh_tx.send(s_hats_w).unwrap();
                });
            } else {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let s_hats_w: Result<Vec<(F, F)>> = (|| {
                        let s_hats = receiver
                            .receive(channel, &inputs, &mut trng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        let _fk = sender
                            .send(channel, &points, inputs.len(), &mut trng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        Ok(s_hats)
                    })();
                    sh_tx.send(s_hats_w).unwrap();
                });
            };
        }

        for s_hats in s_hats_rx.iter().take(channels.len()) {
            let s_hats = s_hats?;
            for (k, &(_, s_hats)) in s_hats.iter().enumerate() {
                s_hat_sum[k] += s_hats;
            }
        }

        Ok(s_hat_sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel_utils::sync_channel::create_unix_channels;
    use crate::channel_utils::sync_channel_by_cb::create_crossbeam_channels;
    use crate::channel_utils::tcp_channel::{
        create_tcp_channels_for_receiver, create_tcp_channels_for_sender,
    };
    use crate::set_utils::create_sets_without_check;
    use crate::solver::{PaxosSolver, Solver, SolverParams, VandelmondeSolver};
    use crate::vole::{
        LPNVoleReceiver, LPNVoleSender, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM,
        LPN_SETUP_SMALL,
    };
    use scuttlebutt::field::F128b;
    use scuttlebutt::{AbstractChannel, AesRng};
    use std::collections::HashSet;
    use std::time::Instant;

    fn test_protocol_mt_base<S, C>(
        nparties: usize,
        set_size: usize,
        common_size: usize,
        receiver_channels: Vec<(PartyId, C)>,
        channels: Vec<Vec<(PartyId, C)>>,
    ) where
        S: Solver<F128b> + Send + 'static,
        C: AbstractChannel + Sync + Send + 'static,
    {
        let mut rng = AesRng::new();

        let m_size = S::calc_params(set_size).code_length();
        let (setup_param, extend_param) = if m_size < (1 << 17) {
            println!("Small parameters are used.");
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            println!("Medium parameters are used.");
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };

        let (intersection, mut sets): (Vec<F128b>, Vec<Vec<F128b>>) =
            create_sets_without_check(nparties, set_size, common_size, &mut rng).unwrap();

        println!("intersection prepared.");

        let mut receiver_channels = receiver_channels
            .into_iter()
            .map(|(i, c)| (i, Arc::new(Mutex::new(c))))
            .collect::<Vec<_>>();
        let channels = channels
            .into_iter()
            .map(|chs| {
                chs.into_iter()
                    .map(|(i, c)| (i, Arc::new(Mutex::new(c))))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        for (i, mut channels) in channels.into_iter().enumerate() {
            // create and fork senders
            let pid = i + 1;
            let set = Arc::new(sets.pop().unwrap());
            std::thread::spawn(move || {
                let mut rng = AesRng::new();

                // offline phase
                let vole_share_for_s = LPNVoleSender::new(setup_param, extend_param);
                let vole_share_for_r = LPNVoleReceiver::new(setup_param, extend_param);
                let sender = Sender::<F128b, S, _, _>::precomp_mt(
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
                sender.send_mt(set, &mut channels, &mut rng).unwrap();

                println!("sender {} finished.", pid);
            });
        }

        // create and run receiver
        // offline phase
        let vole_share_for_s = LPNVoleSender::new(setup_param, extend_param);
        let vole_share_for_r = LPNVoleReceiver::new(setup_param, extend_param);
        let receiver = Receiver::<F128b, S, _, _>::precomp_mt(
            &mut receiver_channels,
            &mut rng,
            vole_share_for_s,
            vole_share_for_r,
            set_size,
        )
        .unwrap();

        println!("receiver prepared. online phase started.");

        // online phase
        let set = Arc::new(sets.pop().unwrap());

        let start = Instant::now();

        let res = receiver
            .receive_mt(set, &receiver_channels, &mut rng)
            .unwrap();

        let d = start.elapsed();

        println!("receiver finished. online time: {:?}", d);

        let res: HashSet<F128b> = HashSet::from_iter(res);
        let intersection: HashSet<F128b> = HashSet::from_iter(intersection);

        assert_eq!(res, intersection);
    }

    #[test]
    fn test_protocol_mt_vandelmonde_unix_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        // create channels
        let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
        test_protocol_mt_base::<VandelmondeSolver<F128b>, _>(
            nparties,
            set_size,
            common_size,
            receiver_channels,
            channels,
        );
    }

    fn test_protocol_mt_paxos_unix_base(nparties: usize, set_size: usize, common_size: usize) {
        // create channels
        let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
        test_protocol_mt_base::<PaxosSolver<F128b>, _>(
            nparties,
            set_size,
            common_size,
            receiver_channels,
            channels,
        );
    }

    fn test_protocol_mt_paxos_tcp_base(
        nparties: usize,
        set_size: usize,
        common_size: usize,
        base_port: usize,
    ) {
        // create channels
        let handles = (1..nparties)
            .map(|me| {
                std::thread::spawn(move || create_tcp_channels_for_sender(nparties, base_port, me))
            })
            .collect::<Vec<_>>();
        let receiver_channels = create_tcp_channels_for_receiver(nparties, base_port).unwrap();
        let channels = handles
            .into_iter()
            .map(|h| h.join().unwrap().unwrap())
            .collect::<Vec<_>>();
        test_protocol_mt_base::<PaxosSolver<F128b>, _>(
            nparties,
            set_size,
            common_size,
            receiver_channels,
            channels,
        );
    }

    #[test]
    fn test_protocol_mt_paxos_unix_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        test_protocol_mt_paxos_unix_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_unix_middle() {
        let nparties = 5;
        let set_size = 1 << 10;
        let common_size = 1 << 5;
        test_protocol_mt_paxos_unix_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_unix_large() {
        let nparties = 5;
        let set_size = 1 << 20;
        let common_size = 1 << 5;
        test_protocol_mt_paxos_unix_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_tcp_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        test_protocol_mt_paxos_tcp_base(nparties, set_size, common_size, 10000);
    }

    #[test]
    fn test_protocol_mt_paxos_tcp_middle() {
        let nparties = 5;
        let set_size = 1 << 10;
        let common_size = 1 << 5;
        test_protocol_mt_paxos_tcp_base(nparties, set_size, common_size, 15000);
    }

    #[test]
    fn test_protocol_mt_paxos_tcp_large() {
        let nparties = 5;
        let set_size = 1 << 20;
        let common_size = 1 << 5;
        test_protocol_mt_paxos_tcp_base(nparties, set_size, common_size, 20000);
    }

    fn test_protocol_mt_paxos_crossbeam_base(nparties: usize, set_size: usize, common_size: usize) {
        // create channels
        let (receiver_channels, channels) = create_crossbeam_channels(nparties);
        test_protocol_mt_base::<PaxosSolver<F128b>, _>(
            nparties,
            set_size,
            common_size,
            receiver_channels,
            channels,
        );
    }

    #[test]
    fn test_protocol_mt_paxos_crossbeam_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        test_protocol_mt_paxos_crossbeam_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_crossbeam_middle() {
        let nparties = 5;
        let set_size = 1 << 10;
        let common_size = 1 << 5;
        test_protocol_mt_paxos_crossbeam_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_crossbeam_large() {
        let nparties = 5;
        let set_size = 1 << 20;
        let common_size = 1 << 5;
        test_protocol_mt_paxos_crossbeam_base(nparties, set_size, common_size);
    }
}
