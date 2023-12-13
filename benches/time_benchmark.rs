use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main};
use preprocessing_mpsi_with_vole::solver::VandelmondeSolver;
use preprocessing_mpsi_with_vole::vole::{
    LPNVoleReceiver, LPNVoleSender, LPN_EXTEND_SMALL, LPN_SETUP_SMALL,
};
use scuttlebutt::field::F128b;
use std::cell::RefCell;
use std::rc::Rc;
mod time_common;
use time_common::{kmprt_tcp_fn, kmprt_unix_fn, preprocessed_tcp_fn, preprocessed_unix_fn};

fn bench_kmprt(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 10;

    let mut group = c.benchmark_group("kmprt_time");
    for e in min_e..=max_e {
        let size: usize = 1 << e;
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("UnixStream", size),
            &size,
            kmprt_unix_fn(nparties),
        );
        let base_port_rc: Rc<RefCell<usize>> = Rc::new(RefCell::new(10000));
        group.bench_with_input(
            BenchmarkId::new("TcpStream", size),
            &size,
            kmprt_tcp_fn(nparties, base_port_rc),
        );
    }
    group.finish();
}

fn bench_preprocessed_svole_poly(c: &mut Criterion) {
    let nparties = 5;
    let min_e = 3;
    let max_e = 8;

    let mut group = c.benchmark_group("preprocessed_svole_poly_time");
    for e in min_e..=max_e {
        let size: usize = 1 << e;
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("UnixStream", size),
            &size,
            preprocessed_unix_fn::<F128b, VandelmondeSolver<F128b>, _, _>(
                nparties,
                LPNVoleSender::new(LPN_SETUP_SMALL, LPN_EXTEND_SMALL),
                LPNVoleReceiver::new(LPN_SETUP_SMALL, LPN_EXTEND_SMALL),
            ),
        );
        let base_port_rc: Rc<RefCell<usize>> = Rc::new(RefCell::new(10000));
        group.bench_with_input(
            BenchmarkId::new("TcpStream", size),
            &size,
            preprocessed_tcp_fn::<F128b, VandelmondeSolver<F128b>, _, _>(
                nparties,
                LPNVoleSender::new(LPN_SETUP_SMALL, LPN_EXTEND_SMALL),
                LPNVoleReceiver::new(LPN_SETUP_SMALL, LPN_EXTEND_SMALL),
                base_port_rc,
            ),
        );
    }
    group.finish();
}

criterion_group!(
    name = time_benches;
    config = Criterion::default().sample_size(10);
    targets = bench_kmprt, bench_preprocessed_svole_poly
);
criterion_main!(time_benches);

// cargo bench kmprt_time
// cargo bench preprocessed_svole_poly_time
