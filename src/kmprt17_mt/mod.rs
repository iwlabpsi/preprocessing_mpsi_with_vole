use anyhow::{Context, Result};
use itertools::Itertools;
use ocelot::oprf::{KmprtReceiver, KmprtSender};
use rand::Rng;
use scuttlebutt::{AbstractChannel, AesRng, Block, Block512};
use std::sync::{mpsc::channel, Arc, Mutex};

pub type PartyId = usize;

struct MultiThreadParty {
    id: PartyId,
    opprf_senders: Vec<(usize, Arc<Mutex<KmprtSender>>)>,
    opprf_receivers: Vec<(usize, Arc<Mutex<KmprtReceiver>>)>,
}

pub struct MultiThreadSender(MultiThreadParty);

pub struct MultiThreadReceiver(MultiThreadParty);

impl MultiThreadSender {
    pub fn init<C>(
        me: PartyId,
        channels: &mut [(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<Self>
    where
        C: AbstractChannel + Send + 'static,
    {
        MultiThreadParty::init(me, channels, rng).map(Self)
    }

    pub fn send<C>(
        &mut self,
        inputs: Arc<Vec<Block>>,
        channels: &[(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<()>
    where
        C: AbstractChannel + Send + 'static,
    {
        assert!(self.0.id != 0);

        // conditional zero sharing
        let inpts = Arc::clone(&inputs);
        let s_hat_sum = self.0.conditional_secret_sharing(inpts, channels, rng)?;

        // conditional reconstruction
        let points = inputs
            .iter()
            .cloned()
            .zip(s_hat_sum.into_iter())
            .collect_vec();

        let mut ch = channels[0].1.lock().unwrap();
        let channel: &mut C = &mut ch;
        self.0.opprf_senders[0]
            .1
            .lock()
            .unwrap()
            .send(channel, &points, inputs.len(), rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok(())
    }
}

impl MultiThreadReceiver {
    pub fn init<C>(channels: &mut [(PartyId, Arc<Mutex<C>>)], rng: &mut AesRng) -> Result<Self>
    where
        C: AbstractChannel + Send + 'static,
    {
        MultiThreadParty::init(0, channels, rng).map(Self)
    }

    pub fn receive<C>(
        &mut self,
        inputs: Arc<Vec<Block>>,
        channels: &[(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<Vec<Block>>
    where
        C: AbstractChannel + Send + 'static,
    {
        // conditional zero sharing
        let inpts = Arc::clone(&inputs);
        let mut s_hat_sum = self
            .0
            .conditional_secret_sharing(inpts, channels, rng)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        let (share_tx, share_rx) = channel();

        // conditional reconstruction
        for ((them, channel), (ri, receiver)) in channels.iter().zip(self.0.opprf_receivers.iter())
        {
            assert_eq!(them, ri);
            let ch = Arc::clone(channel);
            let s_tx = share_tx.clone();
            let mut rng = rng.fork();
            let inputs = Arc::clone(&inputs);
            let receiver = Arc::clone(receiver);

            std::thread::spawn(move || {
                let mut ch = ch.lock().unwrap();
                let channel: &mut C = &mut ch;
                let mut receiver = receiver.lock().unwrap();
                let shares = receiver
                    .receive(channel, &inputs, &mut rng)
                    .with_context(|| format!("@{}:{}", file!(), line!()));
                s_tx.send(shares).unwrap();
            });
        }

        for shares in share_rx.iter().take(channels.len()) {
            let shares = shares?;
            for (i, share) in shares.into_iter().enumerate() {
                s_hat_sum[i] ^= share;
            }
        }

        let intersection = inputs
            .iter()
            .zip(s_hat_sum.into_iter())
            .filter_map(|(x, s)| {
                if s == Block512::default() {
                    Some(*x)
                } else {
                    None
                }
            })
            .collect_vec();

        Ok(intersection)
    }
}

impl MultiThreadParty {
    fn init<C>(
        me: PartyId,
        channels: &mut [(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<Self>
    where
        C: AbstractChannel + Send + 'static,
    {
        let (sender_tx, sender_rx) = channel();
        let (receiver_tx, receiver_rx) = channel();

        channels.sort_by_key(|(them, _)| *them);

        for (them, channel) in channels.iter_mut() {
            let mut rng = rng.fork();
            let ch = Arc::clone(channel);
            let s_tx = sender_tx.clone();
            let r_tx = receiver_tx.clone();

            // the party with the lowest PID gets to initialize their OPPRF sender first
            let them = *them;
            if me < them {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let sndr = KmprtSender::init(channel, &mut rng)
                        .with_context(|| format!("@{}:{}", file!(), line!()));
                    s_tx.send((them, sndr)).unwrap();
                    let rcvr = KmprtReceiver::init(channel, &mut rng)
                        .with_context(|| format!("@{}:{}", file!(), line!()));
                    r_tx.send((them, rcvr)).unwrap();
                });
            } else {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let rcvr = KmprtReceiver::init(channel, &mut rng)
                        .with_context(|| format!("@{}:{}", file!(), line!()));
                    r_tx.send((them, rcvr)).unwrap();
                    let sndr = KmprtSender::init(channel, &mut rng)
                        .with_context(|| format!("@{}:{}", file!(), line!()));
                    s_tx.send((them, sndr)).unwrap();
                });
            }
        }

        let mut opprf_senders = (0..channels.len())
            .map(|_| {
                let (i, s) = sender_rx.recv().unwrap();
                Ok((i, Arc::new(Mutex::new(s?))))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut opprf_receivers = (0..channels.len())
            .map(|_| {
                let (i, r) = receiver_rx.recv().unwrap();
                Ok((i, Arc::new(Mutex::new(r?))))
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

    fn conditional_secret_sharing<C>(
        &mut self,
        inputs: Arc<Vec<Block>>,
        channels: &[(PartyId, Arc<Mutex<C>>)],
        rng: &mut AesRng,
    ) -> Result<Vec<Block512>>
    where
        C: AbstractChannel + Send + 'static,
    {
        let nparties = channels.len() + 1;
        let ninputs = inputs.len();

        let mut s_hat_sum = vec![Block512::default(); ninputs];

        let s = (0..ninputs)
            .map(|i| {
                let shares = secret_sharing_of_zero(nparties, rng);
                s_hat_sum[i] = shares[self.id];
                shares
            })
            .collect_vec();

        let (s_hats_tx, s_hats_rx) = channel();

        for (((other_id, channel), (si, sender)), (ri, receiver)) in channels
            .iter()
            .zip(self.opprf_senders.iter())
            .zip(self.opprf_receivers.iter())
        {
            assert_eq!(other_id, si);
            assert_eq!(other_id, ri);

            let points = inputs
                .iter()
                .enumerate()
                .map(|(k, &x)| (x, s[k][*other_id]))
                .collect_vec();
            let mut rng = rng.fork();
            let ch = Arc::clone(channel);
            let sh_tx = s_hats_tx.clone();
            let inputs = Arc::clone(&inputs);
            let sender = Arc::clone(sender);
            let receiver = Arc::clone(receiver);

            if self.id < *other_id {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let mut sender = sender.lock().unwrap();
                    let mut receiver = receiver.lock().unwrap();
                    let s_hats_w: Result<Vec<Block512>> = (|| {
                        sender
                            .send(channel, &points, inputs.len(), &mut rng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        let s_hats = receiver
                            .receive(channel, &inputs, &mut rng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        Ok(s_hats)
                    })();
                    sh_tx.send(s_hats_w).unwrap();
                });
            } else {
                std::thread::spawn(move || {
                    let mut ch = ch.lock().unwrap();
                    let channel: &mut C = &mut ch;
                    let mut sender = sender.lock().unwrap();
                    let mut receiver = receiver.lock().unwrap();
                    let s_hats_w: Result<Vec<Block512>> = (|| {
                        let s_hats = receiver
                            .receive(channel, &inputs, &mut rng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        sender
                            .send(channel, &points, inputs.len(), &mut rng)
                            .with_context(|| format!("@{}:{}", file!(), line!()))?;
                        Ok(s_hats)
                    })();
                    sh_tx.send(s_hats_w).unwrap();
                });
            }
        }

        for s_hats in s_hats_rx.iter().take(channels.len()) {
            let s_hats = s_hats?;
            for (k, &s_hats) in s_hats.iter().enumerate() {
                s_hat_sum[k] ^= s_hats;
            }
        }

        Ok(s_hat_sum)
    }
}

fn secret_sharing_of_zero<R: Rng>(nparties: usize, rng: &mut R) -> Vec<Block512> {
    let mut sum = Block512::default();
    let mut shares = (0..nparties - 1)
        .map(|_| {
            let b = rng.gen();
            sum ^= b;
            b
        })
        .collect_vec();
    shares.push(sum);
    shares
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel_utils::sync_channel::create_unix_channels;
    use crate::set_utils::create_sets_without_check;
    use scuttlebutt::{AesRng, Block};
    use std::collections::HashSet;

    fn test_protocol_mt_base(nparties: usize, set_size: usize, common_size: usize) {
        let mut rng = AesRng::new();

        let (intersection, mut sets): (Vec<Block>, Vec<Vec<Block>>) =
            create_sets_without_check(nparties, set_size, common_size, &mut rng).unwrap();

        println!("intersection prepared.");

        // create channels
        let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();

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
                let mut sender = MultiThreadSender::init(pid, &mut channels, &mut rng).unwrap();

                println!("sender {} initialized.", pid);

                // online phase
                sender.send(set, &mut channels, &mut rng).unwrap();

                println!("sender {} finished.", pid);
            });
        }

        // create and run receiver
        // offline phase
        let mut receiver = MultiThreadReceiver::init(&mut receiver_channels, &mut rng).unwrap();

        println!("receiver initialized.");

        // online phase
        let set = Arc::new(sets.pop().unwrap());
        let res = receiver.receive(set, &receiver_channels, &mut rng).unwrap();

        println!("receiver finished.");

        let res: HashSet<Block> = HashSet::from_iter(res);
        let intersection: HashSet<Block> = HashSet::from_iter(intersection);

        assert_eq!(res, intersection);
    }

    #[test]
    fn test_protocol_mt_vandelmonde_small() {
        let nparties = 3;
        let set_size = 10;
        let common_size = 5;
        test_protocol_mt_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_middle() {
        let nparties = 5;
        let set_size = 1 << 10;
        let common_size = 1 << 5;
        test_protocol_mt_base(nparties, set_size, common_size);
    }

    #[test]
    fn test_protocol_mt_paxos_large() {
        let nparties = 5;
        let set_size = 1 << 20;
        let common_size = 1 << 5;
        test_protocol_mt_base(nparties, set_size, common_size);
    }
}
