use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main};
use criterion::{AxisScale, PlotConfiguration};
use preprocessing_mpsi_with_vole::set_utils::create_sets_random;
use preprocessing_mpsi_with_vole::solver::Solver;
use preprocessing_mpsi_with_vole::solver::{PaxosSolver, SolverParams};
use preprocessing_mpsi_with_vole::vole::{
    LPNVoleReceiver, LPNVoleSender, OtVoleReceiver, OtVoleSender, LPN_EXTEND_MEDIUM,
    LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM, LPN_SETUP_SMALL,
};
use scuttlebutt::field::F128b;
use scuttlebutt::{AesRng, Block};
use std::cell::RefCell;
use std::rc::Rc;
mod time_common_mt;
use ocelot::ot::{AlszReceiver as OtReceiver, AlszSender as OtSender};
use std::sync::Arc;
use time_common_mt::{
    kmprt_mt_tcp_fn, kmprt_mt_unix_fn, preprocessed_mt_tcp_fn, preprocessed_mt_unix_fn,
    preprocessed_mt_with_offline_tcp_fn, preprocessed_mt_with_offline_unix_fn,
};

fn bench_unix_mt_base(
    c: &mut Criterion,
    // secs: u64,
    nparties: usize,
    min_e: usize,
    max_e: usize,
    name: &str,
) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group(format!("time_unix_stream_mt_compare_{}", name));
    group.plot_config(plot_config);
    // group.measurement_time(Duration::from_secs(secs));
    for e in min_e..=max_e {
        if e >= 14 {
            group.sample_size(10);
        }

        let size: usize = 1 << e;

        let m = PaxosSolver::<F128b>::calc_params(size).code_length();
        let (setup_param, extend_param) = if m < (1 << 17) {
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };

        let mut rng = AesRng::new();
        let (_common, sets): (Vec<Block>, _) =
            create_sets_random(nparties, size, &mut rng).unwrap();
        let sets_for_kmprt = sets.into_iter().map(Arc::new).collect::<Vec<_>>();
        let (_common, sets): (Vec<F128b>, _) =
            create_sets_random(nparties, size, &mut rng).unwrap();
        let sets_for_prep = sets.into_iter().map(Arc::new).collect::<Vec<_>>();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(
            BenchmarkId::new("KMPRT17_Multithread", size),
            kmprt_mt_unix_fn(nparties, sets_for_kmprt.clone()),
        );
        group.bench_function(
            BenchmarkId::new("Preprocessing_Paxos_Multithread", size),
            preprocessed_mt_unix_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                sets_for_prep.clone(),
                size,
                LPNVoleSender::new(setup_param, extend_param),
                LPNVoleReceiver::new(setup_param, extend_param),
            ),
        );
        group.bench_function(
            BenchmarkId::new("Preprocessing_offLPN_Paxos_Multithread", size),
            preprocessed_mt_with_offline_unix_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                sets_for_prep.clone(),
                size,
                LPNVoleSender::new(setup_param, extend_param),
                LPNVoleReceiver::new(setup_param, extend_param),
            ),
        );
        group.bench_function(
            BenchmarkId::new("Preprocessing_offOT_Paxos_Multithread", size),
            preprocessed_mt_with_offline_unix_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                sets_for_prep.clone(),
                size,
                OtVoleSender::<F128b, 128, OtSender>::new(),
                OtVoleReceiver::<F128b, 128, OtReceiver>::new(),
            ),
        );
    }
    group.finish();
}

fn bench_unix_mt_short(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 5;
    bench_unix_mt_base(c, nparties, min_e, max_e, "short");
}

fn bench_unix_mt_middle(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 12;
    bench_unix_mt_base(c, nparties, min_e, max_e, "middle");
}

fn bench_unix_mt_long(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 20;
    bench_unix_mt_base(c, nparties, min_e, max_e, "long");
}

fn bench_tcp_mt_base(
    c: &mut Criterion,
    // secs: u64,
    nparties: usize,
    min_e: usize,
    max_e: usize,
    name: &str,
) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group(format!("time_tcp_stream_mt_compare_{}", name));
    group.plot_config(plot_config);
    // group.measurement_time(Duration::from_secs(secs));
    for e in min_e..=max_e {
        if e >= 14 {
            group.sample_size(10);
        }

        let size: usize = 1 << e;

        let m = PaxosSolver::<F128b>::calc_params(size).code_length();
        let (setup_param, extend_param) = if m < (1 << 17) {
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };

        let mut rng = AesRng::new();
        let (_common, sets): (Vec<Block>, _) =
            create_sets_random(nparties, size, &mut rng).unwrap();
        let sets_for_kmprt = sets.into_iter().map(Arc::new).collect::<Vec<_>>();
        let (_common, sets): (Vec<F128b>, _) =
            create_sets_random(nparties, size, &mut rng).unwrap();
        let sets_for_prep = sets.into_iter().map(Arc::new).collect::<Vec<_>>();

        group.throughput(Throughput::Elements(size as u64));
        let base_port_rc: Rc<RefCell<usize>> = Rc::new(RefCell::new(10000));
        group.bench_function(
            BenchmarkId::new("KMPRT17_Multithread", size),
            kmprt_mt_tcp_fn(nparties, sets_for_kmprt, base_port_rc),
        );
        let base_port_rc: Rc<RefCell<usize>> = Rc::new(RefCell::new(20000));
        group.bench_function(
            BenchmarkId::new("Preprocessing_Paxos_Multithread", size),
            preprocessed_mt_tcp_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                sets_for_prep.clone(),
                size,
                LPNVoleSender::new(setup_param, extend_param),
                LPNVoleReceiver::new(setup_param, extend_param),
                base_port_rc.clone(),
            ),
        );
        group.bench_function(
            BenchmarkId::new("Preprocessing_offLPN_Paxos_Multithread", size),
            preprocessed_mt_with_offline_tcp_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                sets_for_prep.clone(),
                size,
                LPNVoleSender::new(setup_param, extend_param),
                LPNVoleReceiver::new(setup_param, extend_param),
                base_port_rc.clone(),
            ),
        );
        group.bench_function(
            BenchmarkId::new("Preprocessing_offOT_Paxos_Multithread", size),
            preprocessed_mt_with_offline_tcp_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                sets_for_prep,
                size,
                OtVoleSender::<F128b, 128, OtSender>::new(),
                OtVoleReceiver::<F128b, 128, OtReceiver>::new(),
                base_port_rc,
            ),
        );
    }
    group.finish();
}

fn bench_tcp_mt_short(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 5;
    bench_tcp_mt_base(c, nparties, min_e, max_e, "short");
}

fn bench_tcp_mt_middle(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 12;
    bench_tcp_mt_base(c, nparties, min_e, max_e, "middle");
}

fn bench_tcp_mt_long(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 20;
    bench_tcp_mt_base(c, nparties, min_e, max_e, "long");
}

criterion_group!(
    name = time_benches_mt_compare;
    config = Criterion::default();
    targets = bench_unix_mt_short, bench_unix_mt_middle, bench_unix_mt_long, bench_tcp_mt_short, bench_tcp_mt_middle, bench_tcp_mt_long
);
criterion_main!(time_benches_mt_compare);
