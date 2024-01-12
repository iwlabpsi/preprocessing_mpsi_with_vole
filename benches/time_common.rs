use criterion::Bencher;
use popsicle::kmprt::{Receiver as KmprtReceiver, Sender as KmprtSender};
use preprocessing_mpsi_with_vole::channel_utils::sync_channel::create_unix_channels;
use preprocessing_mpsi_with_vole::channel_utils::tcp_channel::{
    create_tcp_channels_for_receiver, create_tcp_channels_for_sender,
};
use preprocessing_mpsi_with_vole::preprocessed::psi::{
    Receiver as SepReceiver, Sender as SepSender,
};
use preprocessing_mpsi_with_vole::set_utils::{create_sets_random, FromU128};
use preprocessing_mpsi_with_vole::solver::Solver;
use preprocessing_mpsi_with_vole::vole::{VoleShareForReceiver, VoleShareForSender};
use rand::distributions::{Distribution, Standard};
use scuttlebutt::{field::FiniteField as FF, AesRng, Block, SyncChannel};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[allow(unused)]
fn kmprt_routine<R, W>(
    mut sets: Vec<Vec<Block>>,
    mut receiver_channels: Vec<(usize, SyncChannel<R, W>)>,
    channels: Vec<Vec<(usize, SyncChannel<R, W>)>>,
) -> Duration
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    let recv_set = sets.pop().unwrap();
    let mut rngs = vec![AesRng::new(); channels.len()];
    let mut rng = AesRng::new();

    let mut handles = Vec::new();
    for (i, mut channels) in channels.into_iter().enumerate() {
        let i = i + 1;
        let set = sets.pop().unwrap();
        let mut rng = rngs.pop().unwrap();
        let handle = std::thread::spawn(move || {
            let mut sender = KmprtSender::init(i, &mut channels, &mut rng).unwrap();
            sender.send(&set, &mut channels, &mut rng).unwrap();
        });
        handles.push(handle);
    }

    let mut receiver = KmprtReceiver::init(&mut receiver_channels, &mut rng).unwrap();
    let start = Instant::now();
    let _res = receiver
        .receive(&recv_set, &mut receiver_channels, &mut rng)
        .unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    start.elapsed()
}

#[allow(unused)]
pub(crate) fn kmprt_unix_fn(nparties: usize) -> impl FnMut(&mut Bencher<'_>, &usize) {
    move |b, &size| {
        b.iter_custom(|iter| {
            let mut rng = AesRng::new();
            let (_common, sets): (Vec<Block>, _) =
                create_sets_random(nparties, size, &mut rng).unwrap();
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
                let sets = sets.clone();
                // let common = common.clone();

                total_time += kmprt_routine(sets, receiver_channels, channels);
            }

            total_time
        });
    }
}

#[allow(unused)]
pub(crate) fn kmprt_tcp_fn(
    nparties: usize,
    base_port_rc: Rc<RefCell<usize>>,
) -> impl FnMut(&mut Bencher<'_>, &usize) {
    move |b, &size| {
        let bport_rc = Rc::clone(&base_port_rc);
        b.iter_custom(|iter| {
            let mut rng = AesRng::new();
            let (_common, sets): (Vec<Block>, _) =
                create_sets_random(nparties, size, &mut rng).unwrap();
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let base_port = {
                    let mut base_port_mut = bport_rc.borrow_mut();
                    let now = *base_port_mut;
                    *base_port_mut += nparties;
                    now
                };
                let handles = (1..nparties)
                    .map(|me| {
                        std::thread::spawn(move || {
                            create_tcp_channels_for_sender(nparties, base_port, me)
                        })
                    })
                    .collect::<Vec<_>>();
                let receiver_channels =
                    create_tcp_channels_for_receiver(nparties, base_port).unwrap();
                let channels = handles
                    .into_iter()
                    .map(|h| h.join().unwrap().unwrap())
                    .collect::<Vec<_>>();

                let sets = sets.clone();
                // let common = common.clone();

                total_time += kmprt_routine(sets, receiver_channels, channels);
            }

            total_time
        });
    }
}

#[allow(unused)]
fn preprocessed_routine<R, W, F, S, VS, VR>(
    mut sets: Vec<Vec<F>>,
    mut receiver_channels: Vec<(usize, SyncChannel<R, W>)>,
    channels: Vec<Vec<(usize, SyncChannel<R, W>)>>,
    receiver: SepReceiver<F, S, VS, VR>,
    mut senders: Vec<SepSender<F, S, VS, VR>>,
) -> Duration
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
    F: FF,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    let recv_set = sets.pop().unwrap();
    let mut rngs = vec![AesRng::new(); channels.len()];
    let mut rng = AesRng::new();

    let mut handles = Vec::new();
    for (i, mut channels) in channels.into_iter().enumerate() {
        let me = i + 1;
        let set = sets.pop().unwrap();
        let mut rng = rngs.pop().unwrap();
        let sender = senders.remove(0);
        assert!(sender.get_id() == me);
        let handle = std::thread::spawn(move || {
            sender.send(&set, &mut channels, &mut rng).unwrap();
        });
        handles.push(handle);
    }

    let start = Instant::now();
    let _res = receiver
        .receive(&recv_set, &mut receiver_channels, &mut rng)
        .unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    start.elapsed()
}

#[allow(unused)]
fn create_parties<F, S, VS, VR>(
    nparties: usize,
    set_size: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
) -> (SepReceiver<F, S, VS, VR>, Vec<SepSender<F, S, VS, VR>>)
where
    F: FF,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    let (mut receiver_channels, channels) = create_unix_channels(nparties).unwrap();

    let mut handles = Vec::with_capacity(nparties - 1);
    for (i, mut channels) in channels.into_iter().enumerate() {
        // create and fork senders
        let pid = i + 1;
        let vole_share_for_s = vole_share_for_s.clone();
        let vole_share_for_r = vole_share_for_r.clone();
        handles.push(std::thread::spawn(move || {
            let mut rng = AesRng::new();

            // offline phase
            let sender = SepSender::<F, S, _, _>::precomp(
                pid,
                &mut channels,
                &mut rng,
                vole_share_for_s,
                vole_share_for_r,
                set_size,
            )
            .unwrap();

            sender
        }));
    }

    let mut rng = AesRng::new();
    let receiver = SepReceiver::<F, S, _, _>::precomp(
        &mut receiver_channels,
        &mut rng,
        vole_share_for_s,
        vole_share_for_r,
        set_size,
    )
    .unwrap();

    let senders = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();

    (receiver, senders)
}

#[allow(unused)]
pub(crate) fn preprocessed_unix_fn<F, S, VS, VR>(
    nparties: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
) -> impl FnMut(&mut Bencher<'_>, &usize)
where
    F: FF + FromU128,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    move |b, &size| {
        let (receiver, senders) =
            create_parties(nparties, size, vole_share_for_s, vole_share_for_r);

        b.iter_custom(move |iter| {
            let mut rng = AesRng::new();
            let (_common, sets): (Vec<F>, _) =
                create_sets_random(nparties, size, &mut rng).unwrap();
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();

                let sets = sets.clone();
                let receiver: SepReceiver<F, S, VS, VR> = receiver.clone();
                let senders: Vec<SepSender<F, S, VS, VR>> = senders.clone();

                total_time +=
                    preprocessed_routine(sets, receiver_channels, channels, receiver, senders);
            }

            total_time
        });
    }
}

#[allow(unused)]
pub(crate) fn preprocessed_tcp_fn<F, S, VS, VR>(
    nparties: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
    base_port_rc: Rc<RefCell<usize>>,
) -> impl FnMut(&mut Bencher<'_>, &usize)
where
    F: FF + FromU128,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    move |b, &size| {
        let bport_rc = Rc::clone(&base_port_rc);
        let (receiver, senders) =
            create_parties(nparties, size, vole_share_for_s, vole_share_for_r);

        b.iter_custom(move |iter| {
            let mut rng = AesRng::new();
            let (_common, sets): (Vec<F>, _) =
                create_sets_random(nparties, size, &mut rng).unwrap();
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let base_port = {
                    let mut base_port_mut = bport_rc.borrow_mut();
                    let now = *base_port_mut;
                    *base_port_mut += nparties;
                    now
                };
                let handles = (1..nparties)
                    .map(|me| {
                        std::thread::spawn(move || {
                            create_tcp_channels_for_sender(nparties, base_port, me)
                        })
                    })
                    .collect::<Vec<_>>();
                let receiver_channels =
                    create_tcp_channels_for_receiver(nparties, base_port).unwrap();
                let channels = handles
                    .into_iter()
                    .map(|h| h.join().unwrap().unwrap())
                    .collect::<Vec<_>>();

                let sets = sets.clone();
                let receiver: SepReceiver<F, S, VS, VR> = receiver.clone();
                let senders: Vec<SepSender<F, S, VS, VR>> = senders.clone();

                total_time +=
                    preprocessed_routine(sets, receiver_channels, channels, receiver, senders);
            }

            total_time
        });
    }
}
