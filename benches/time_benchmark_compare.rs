use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main};
use preprocessing_mpsi_with_vole::solver::Solver;
use preprocessing_mpsi_with_vole::solver::{PaxosSolver, SolverParams, VandelmondeSolver};
use preprocessing_mpsi_with_vole::vole::{
    LPNVoleReceiver, LPNVoleSender, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM,
    LPN_SETUP_SMALL,
};
use scuttlebutt::field::F128b;
use std::cell::RefCell;
use std::rc::Rc;
mod time_common;
use time_common::{kmprt_tcp_fn, kmprt_unix_fn, preprocessed_tcp_fn, preprocessed_unix_fn};

fn bench_unix(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 12;

    let mut group = c.benchmark_group("time_unix_stream_compare");
    for e in min_e..=max_e {
        let size: usize = 1 << e;

        let m = PaxosSolver::<F128b>::calc_params(size).code_length();
        let (setup_param, extend_param) = if m < (1 << 17) {
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("KMPRT", size),
            &size,
            kmprt_unix_fn(nparties),
        );
        group.bench_with_input(
            BenchmarkId::new("Preprocessing_Paxos", size),
            &size,
            preprocessed_unix_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                LPNVoleSender::new(setup_param, extend_param),
                LPNVoleReceiver::new(setup_param, extend_param),
            ),
        );
        /*
        group.bench_with_input(
            BenchmarkId::new("Preprocessing_poly", size),
            &size,
            preprocessed_unix_fn::<F128b, VandelmondeSolver<F128b>, _, _>(
                nparties,
                LPNVoleSender::new(LPN_SETUP_SMALL, LPN_EXTEND_SMALL),
                LPNVoleReceiver::new(LPN_SETUP_SMALL, LPN_EXTEND_SMALL),
            ),
        );
        */
    }
    group.finish();
}

fn bench_tcp(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 12;

    let mut group = c.benchmark_group("time_tcp_stream_compare");
    for e in min_e..=max_e {
        let size: usize = 1 << e;

        let m = PaxosSolver::<F128b>::calc_params(size).code_length();
        let (setup_param, extend_param) = if m < (1 << 17) {
            (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
        } else {
            (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
        };

        group.throughput(Throughput::Elements(size as u64));
        let base_port_rc: Rc<RefCell<usize>> = Rc::new(RefCell::new(10000));
        group.bench_with_input(
            BenchmarkId::new("KMPRT", size),
            &size,
            kmprt_tcp_fn(nparties, base_port_rc),
        );
        let base_port_rc: Rc<RefCell<usize>> = Rc::new(RefCell::new(20000));
        group.bench_with_input(
            BenchmarkId::new("Preprocessing_Paxos", size),
            &size,
            preprocessed_tcp_fn::<F128b, PaxosSolver<F128b>, _, _>(
                nparties,
                LPNVoleSender::new(setup_param, extend_param),
                LPNVoleReceiver::new(setup_param, extend_param),
                base_port_rc,
            ),
        );
    }
    group.finish();
}

criterion_group!(
    name = time_benches_compare;
    config = Criterion::default().sample_size(20);
    targets = bench_unix, bench_tcp
);
criterion_main!(time_benches_compare);
