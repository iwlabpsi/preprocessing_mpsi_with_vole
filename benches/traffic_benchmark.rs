fn main() {}

/*
// I tried to use the following code to benchmark the traffic of kmprt.
// However, it seems that the traffic cannot be measured by criterion.rs .

mod measure_utils;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main};
use measure_utils::{
    create_readcount_channels, create_writecount_channels, TrafficBytes, TrafficBytesMeasurement,
};
// use preprocessing_mpsi_with_vole::preprocessed::psi::{Receiver as SepReceiver, Sender as SepSender};
use preprocessing_mpsi_with_vole::set_utils::create_sets_random;
// use preprocessing_mpsi_with_vole::solver::VandelmondeSolver;
// use preprocessing_mpsi_with_vole::vole::{LPNVoleSender, VoleShareForSender, LPN_EXTEND_SMALL, LPN_SETUP_SMALL};
use popsicle::kmprt::{Receiver as KmprtReceiver, Sender as KmprtSender};
use scuttlebutt::SyncChannel;
use scuttlebutt::{AesRng, Block};
use std::io::{Read, Write};

fn kmprt_routine<R, W>(
    sets: Vec<Vec<Block>>,
    mut receiver_channels: Vec<(usize, SyncChannel<R, W>)>,
    channels: Vec<Vec<(usize, SyncChannel<R, W>)>>,
) where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    let mut rng = AesRng::new();
    let mut sets = sets.clone();

    let recv_set = sets.pop().unwrap();

    let mut handles = Vec::new();
    for (i, mut channels) in channels.into_iter().enumerate() {
        let i = i + 1;
        let set = sets.pop().unwrap();
        let handle = std::thread::spawn(move || {
            let mut rng = AesRng::new();
            let mut sender = KmprtSender::init(i, &mut channels, &mut rng).unwrap();
            sender.send(&set, &mut channels, &mut rng).unwrap();
        });
        handles.push(handle);
    }

    let mut receiver = KmprtReceiver::init(&mut receiver_channels, &mut rng).unwrap();
    let _res = receiver
        .receive(&recv_set, &mut receiver_channels, &mut rng)
        .unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
}

fn bench_kmprt(c: &mut Criterion<TrafficBytesMeasurement>) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 10;

    let mut group = c.benchmark_group("kmprt_traffic");
    for e in min_e..=max_e {
        let size: usize = 1 << e;
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("Read", size), &size, move |b, &size| {
            let mut rng = AesRng::new();
            let (_common, sets): (Vec<Block>, _) =
                create_sets_random(nparties, size, &mut rng).unwrap();

            b.iter_custom(move |iter| {
                let mut sum: usize = 0;
                for _ in 0..iter {
                    let sets = sets.clone();
                    let (mut trrafic_bytes, tx) = TrafficBytes::new();
                    let (receiver_channels, channels) =
                        create_readcount_channels(nparties, tx).unwrap();
                    kmprt_routine(sets, receiver_channels, channels);
                    sum += trrafic_bytes.total_bytes();
                }
                sum
            });
        });
        group.bench_with_input(BenchmarkId::new("Write", size), &size, move |b, &size| {
            let mut rng = AesRng::new();
            let (_common, sets): (Vec<Block>, _) =
                create_sets_random(nparties, size, &mut rng).unwrap();

            b.iter_custom(move |iter| {
                let mut sum: usize = 0;
                for _ in 0..iter {
                    let sets = sets.clone();
                    let (mut trrafic_bytes, tx) = TrafficBytes::new();
                    let (receiver_channels, channels) =
                        create_writecount_channels(nparties, tx).unwrap();
                    kmprt_routine(sets, receiver_channels, channels);
                    sum += trrafic_bytes.total_bytes();
                }
                sum
            });
        });
    }
    group.finish();
}

fn criterion() -> Criterion<TrafficBytesMeasurement> {
    let c = Criterion::default().sample_size(10);
    c.with_measurement(TrafficBytesMeasurement)
}

criterion_group!(
    name = traffic_benches;
    config = criterion();
    targets = bench_kmprt
);
criterion_main!(traffic_benches);
*/
