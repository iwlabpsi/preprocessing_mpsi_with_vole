use criterion::Bencher;
use preprocessing_mpsi_with_vole::channel_utils::ch_arcnize;
use preprocessing_mpsi_with_vole::channel_utils::sync_channel::create_unix_channels;
use preprocessing_mpsi_with_vole::channel_utils::tcp_channel::{
    create_tcp_channels_for_receiver, create_tcp_channels_for_sender,
};
use preprocessing_mpsi_with_vole::kmprt17_mt::{
    MultiThreadReceiver as KmprtReceiver, MultiThreadSender as KmprtSender,
};
use preprocessing_mpsi_with_vole::preprocessed::psi::{
    Receiver as SepReceiver, Sender as SepSender,
};
use preprocessing_mpsi_with_vole::set_utils::FromU128;
use preprocessing_mpsi_with_vole::solver::Solver;
use preprocessing_mpsi_with_vole::vole::{VoleShareForReceiver, VoleShareForSender};
use rand::distributions::{Distribution, Standard};
use scuttlebutt::{field::FiniteField as FF, AesRng, Block, SyncChannel};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[allow(unused)]
fn kmprt_mt_routine<R, W>(
    mut sets: Vec<Arc<Vec<Block>>>,
    mut receiver_channels: Vec<(usize, Arc<Mutex<SyncChannel<R, W>>>)>,
    channels: Vec<Vec<(usize, Arc<Mutex<SyncChannel<R, W>>>)>>,
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
            sender.send(set, &channels, &mut rng).unwrap();
        });
        handles.push(handle);
    }

    let mut receiver = KmprtReceiver::init(&mut receiver_channels, &mut rng).unwrap();
    let start = Instant::now();
    let _res = receiver
        .receive(recv_set, &receiver_channels, &mut rng)
        .unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    start.elapsed()
}

#[allow(unused)]
pub(crate) fn kmprt_mt_unix_fn(
    nparties: usize,
    sets: Vec<Arc<Vec<Block>>>,
) -> impl FnMut(&mut Bencher<'_>) {
    move |b| {
        b.iter_custom(|iter| {
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
                let (receiver_channels, channels) =
                    ch_arcnize(receiver_channels, channels).unwrap();
                let sets = sets.clone();
                // let common = common.clone();

                total_time += kmprt_mt_routine(sets, receiver_channels, channels);
            }

            total_time
        });
    }
}

#[allow(unused)]
pub(crate) fn kmprt_mt_tcp_fn(
    nparties: usize,
    sets: Vec<Arc<Vec<Block>>>,
    base_port_rc: Rc<RefCell<usize>>,
) -> impl FnMut(&mut Bencher<'_>) {
    move |b| {
        let bport_rc = Rc::clone(&base_port_rc);
        b.iter_custom(|iter| {
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
                let (receiver_channels, channels) =
                    ch_arcnize(receiver_channels, channels).unwrap();

                let sets = sets.clone();
                // let common = common.clone();

                total_time += kmprt_mt_routine(sets, receiver_channels, channels);
            }

            total_time
        });
    }
}

#[allow(unused)]
fn preprocessed_mt_routine<R, W, F, S, VS, VR>(
    mut sets: Vec<Arc<Vec<F>>>,
    receiver_channels: Vec<(usize, Arc<Mutex<SyncChannel<R, W>>>)>,
    channels: Vec<Vec<(usize, Arc<Mutex<SyncChannel<R, W>>>)>>,
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
    for (i, channels) in channels.into_iter().enumerate() {
        let me = i + 1;
        let set = sets.pop().unwrap();
        let mut rng = rngs.pop().unwrap();
        let sender = senders.remove(0);
        assert!(sender.get_id() == me);
        let handle = std::thread::spawn(move || {
            sender.send_mt(set, &channels, &mut rng).unwrap();
        });
        handles.push(handle);
    }

    let start = Instant::now();
    let _res = receiver
        .receive_mt(recv_set, &receiver_channels, &mut rng)
        .unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    start.elapsed()
}

#[allow(unused)]
fn create_parties_mt<F, S, VS, VR>(
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
    let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
    let (mut receiver_channels, channels) = ch_arcnize(receiver_channels, channels).unwrap();

    let mut handles = Vec::with_capacity(nparties - 1);
    for (i, mut channels) in channels.into_iter().enumerate() {
        // create and fork senders
        let pid = i + 1;
        let vole_share_for_s = vole_share_for_s.clone();
        let vole_share_for_r = vole_share_for_r.clone();
        handles.push(std::thread::spawn(move || {
            let mut rng = AesRng::new();

            // offline phase
            let sender = SepSender::<F, S, _, _>::precomp_mt(
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
    let receiver = SepReceiver::<F, S, _, _>::precomp_mt(
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
pub(crate) fn preprocessed_mt_unix_fn<F, S, VS, VR>(
    nparties: usize,
    sets: Vec<Arc<Vec<F>>>,
    set_size: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
) -> impl FnMut(&mut Bencher<'_>)
where
    F: FF + FromU128,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    move |b| {
        let (receiver, senders) =
            create_parties_mt(nparties, set_size, vole_share_for_s, vole_share_for_r);

        b.iter_custom(|iter| {
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
                let (receiver_channels, channels) =
                    ch_arcnize(receiver_channels, channels).unwrap();

                let sets = sets.clone();
                let receiver: SepReceiver<F, S, VS, VR> = receiver.clone();
                let senders: Vec<SepSender<F, S, VS, VR>> = senders.clone();

                total_time +=
                    preprocessed_mt_routine(sets, receiver_channels, channels, receiver, senders);
            }

            total_time
        });
    }
}

#[allow(unused)]
pub(crate) fn preprocessed_mt_tcp_fn<F, S, VS, VR>(
    nparties: usize,
    sets: Vec<Arc<Vec<F>>>,
    set_size: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
    base_port_rc: Rc<RefCell<usize>>,
) -> impl FnMut(&mut Bencher<'_>)
where
    F: FF + FromU128,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    move |b| {
        let bport_rc = Rc::clone(&base_port_rc);
        let (receiver, senders) =
            create_parties_mt(nparties, set_size, vole_share_for_s, vole_share_for_r);

        b.iter_custom(|iter| {
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
                let (receiver_channels, channels) =
                    ch_arcnize(receiver_channels, channels).unwrap();

                let sets = sets.clone();
                let receiver: SepReceiver<F, S, VS, VR> = receiver.clone();
                let senders: Vec<SepSender<F, S, VS, VR>> = senders.clone();

                total_time +=
                    preprocessed_mt_routine(sets, receiver_channels, channels, receiver, senders);
            }

            total_time
        });
    }
}

#[allow(unused)]
fn preprocessed_mt_with_offline_routine<R, W, F, S, VS, VR>(
    mut sets: Vec<Arc<Vec<F>>>,
    mut receiver_channels: Vec<(usize, Arc<Mutex<SyncChannel<R, W>>>)>,
    channels: Vec<Vec<(usize, Arc<Mutex<SyncChannel<R, W>>>)>>,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
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
        let pid = me;

        let vole_share_for_s = vole_share_for_s.clone();
        let vole_share_for_r = vole_share_for_r.clone();
        let handle = std::thread::spawn(move || {
            // offline phase
            let sender = SepSender::<F, S, _, _>::precomp_mt(
                pid,
                &mut channels,
                &mut rng,
                vole_share_for_s,
                vole_share_for_r,
                set.len(),
            )
            .unwrap();

            // online phase
            sender.send_mt(set, &channels, &mut rng).unwrap();
        });
        handles.push(handle);
    }

    let start = Instant::now();
    let receiver = SepReceiver::<F, S, _, _>::precomp_mt(
        &mut receiver_channels,
        &mut rng,
        vole_share_for_s,
        vole_share_for_r,
        recv_set.len(),
    )
    .unwrap();
    let _res = receiver
        .receive_mt(recv_set, &receiver_channels, &mut rng)
        .unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    start.elapsed()
}

#[allow(unused)]
pub(crate) fn preprocessed_mt_with_offline_unix_fn<F, S, VS, VR>(
    nparties: usize,
    sets: Vec<Arc<Vec<F>>>,
    set_size: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
) -> impl FnMut(&mut Bencher<'_>)
where
    F: FF + FromU128,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    move |b| {
        b.iter_custom(|iter| {
            let mut total_time = Duration::new(0, 0);

            for _ in 0..iter {
                let (receiver_channels, channels) = create_unix_channels(nparties).unwrap();
                let (receiver_channels, channels) =
                    ch_arcnize(receiver_channels, channels).unwrap();

                let sets = sets.clone();

                total_time += preprocessed_mt_with_offline_routine::<_, _, _, S, _, _>(
                    sets,
                    receiver_channels,
                    channels,
                    vole_share_for_s.clone(),
                    vole_share_for_r.clone(),
                );
            }

            total_time
        });
    }
}

#[allow(unused)]
pub(crate) fn preprocessed_mt_with_offline_tcp_fn<F, S, VS, VR>(
    nparties: usize,
    sets: Vec<Arc<Vec<F>>>,
    set_size: usize,
    vole_share_for_s: VS,
    vole_share_for_r: VR,
    base_port_rc: Rc<RefCell<usize>>,
) -> impl FnMut(&mut Bencher<'_>)
where
    F: FF + FromU128,
    S: Solver<F> + Send + 'static,
    VS: VoleShareForSender<F> + Send + 'static,
    VR: VoleShareForReceiver<F> + Send + 'static,
    Standard: Distribution<F>,
{
    move |b| {
        let bport_rc = Rc::clone(&base_port_rc);

        b.iter_custom(|iter| {
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
                let (receiver_channels, channels) =
                    ch_arcnize(receiver_channels, channels).unwrap();

                let sets = sets.clone();

                total_time += preprocessed_mt_with_offline_routine::<_, _, _, S, _, _>(
                    sets,
                    receiver_channels,
                    channels,
                    vole_share_for_s.clone(),
                    vole_share_for_r.clone(),
                );
            }

            total_time
        });
    }
}
