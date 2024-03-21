use crate::channel_utils::ch_arcnize;
use crate::cli_utils::{
    self as cli, create_vole_sr, Args, ChannelUnion, MultiThreadOptimization, SolverType,
    VoleShareForReceiverUnion, VoleShareForSenderUnion,
};
use crate::preprocessed::psi::{Receiver, Sender};
use crate::set_utils::create_sets_without_check;
use crate::solver::{PaxosSolver, VandelmondeSolver};
use anyhow::{Context, Result};
use scuttlebutt::field::F128b;
use scuttlebutt::AesRng;
use std::collections::HashSet;
use std::sync::Arc;

fn intersection_prepare(
    rng: &mut AesRng,
    num_parties: usize,
    set_size: usize,
    common_size: usize,
) -> Result<(Vec<F128b>, Vec<Vec<F128b>>)> {
    let (intersection, sets): (Vec<F128b>, Vec<Vec<F128b>>) =
        create_sets_without_check(num_parties, set_size, common_size, rng)
            .with_context(|| "Failed to create sets.")?;

    println!("intersection prepared.");

    Ok((intersection, sets))
}

fn protocol_base(
    intersection: Vec<F128b>,
    mut sets: Vec<Vec<F128b>>,
    receiver_channels: Vec<(usize, ChannelUnion)>,
    channels: Vec<Vec<(usize, ChannelUnion)>>,
    multi_thread: MultiThreadOptimization,
    solver_type: SolverType,
    vole_share_for_s: VoleShareForSenderUnion,
    vole_share_for_r: VoleShareForReceiverUnion,
) -> Result<()> {
    let r_set = sets.pop().unwrap();

    let handles = channels
        .into_iter()
        .enumerate()
        .map(move |(i, channels)| {
            // create and fork senders
            let pid = i + 1;
            let set = sets.pop().unwrap();
            let vole_share_for_s = vole_share_for_s.clone();
            let vole_share_for_r = vole_share_for_r.clone();
            std::thread::spawn(move || -> Result<()> {
                let mut rng = AesRng::new();

                macro_rules! sender_protocol {
                    ( $chns:expr, $set:expr, $s:path, $send:ident ) => {{
                        let mut chns = $chns;

                        // offline phase
                        // Sender::<F128b, S, _, _>::precomp(
                        let sender = $s(
                            pid,
                            &mut chns,
                            &mut rng,
                            vole_share_for_s,
                            vole_share_for_r,
                            set.len(),
                        )
                        .with_context(|| format!("Failed to create sender {}.", pid))?;

                        println!("sender {} prepared.", pid);

                        // online phase
                        sender
                            .$send($set, &mut chns, &mut rng)
                            .with_context(|| format!("Failed to run sender {}.", pid))?;

                        println!("sender {} finished.", pid);
                    }};
                }

                match (solver_type, multi_thread) {
                    (SolverType::Vandelmonde, MultiThreadOptimization::Off) => {
                        sender_protocol!(
                            channels,
                            &set,
                            Sender::<F128b, VandelmondeSolver<F128b>, _, _>::precomp,
                            send
                        )
                    }
                    (SolverType::Paxos, MultiThreadOptimization::Off) => {
                        sender_protocol!(
                            channels,
                            &set,
                            Sender::<F128b, PaxosSolver<F128b>, _, _>::precomp,
                            send
                        )
                    }
                    (SolverType::Vandelmonde, MultiThreadOptimization::On) => {
                        sender_protocol!(
                            ch_arcnize(channels),
                            Arc::new(set),
                            Sender::<F128b, VandelmondeSolver<F128b>, _, _>::precomp_mt,
                            send_mt
                        )
                    }
                    (SolverType::Paxos, MultiThreadOptimization::On) => {
                        sender_protocol!(
                            ch_arcnize(channels),
                            Arc::new(set),
                            Sender::<F128b, PaxosSolver<F128b>, _, _>::precomp_mt,
                            send_mt
                        )
                    }
                }

                Ok(())
            })
        })
        .collect::<Vec<_>>();

    let mut rng = AesRng::new();

    macro_rules! receiver_protocol {
        ( $chns:expr, $set:expr, $r:path, $receive:ident ) => {{
            let mut chns = $chns;

            // create and run receiver
            // offline phase
            // let receiver = Receiver::<F128b, S, _, _>::precomp(
            let receiver = $r(
                &mut chns,
                &mut rng,
                vole_share_for_s,
                vole_share_for_r,
                r_set.len(),
            )
            .with_context(|| "Failed to create receiver.")?;

            println!("receiver prepared.");

            // online phase
            let res = receiver
                .$receive($set, &mut chns, &mut rng)
                .with_context(|| "Failed to run receiver.")?;

            println!("receiver finished.");

            res
        }};
    }

    let res = match (solver_type, multi_thread) {
        (SolverType::Vandelmonde, MultiThreadOptimization::Off) => {
            receiver_protocol!(
                receiver_channels,
                &r_set,
                Receiver::<F128b, VandelmondeSolver<F128b>, _, _>::precomp,
                receive
            )
        }
        (SolverType::Paxos, MultiThreadOptimization::Off) => {
            receiver_protocol!(
                receiver_channels,
                &r_set,
                Receiver::<F128b, PaxosSolver<F128b>, _, _>::precomp,
                receive
            )
        }
        (SolverType::Vandelmonde, MultiThreadOptimization::On) => {
            receiver_protocol!(
                ch_arcnize(receiver_channels),
                Arc::new(r_set),
                Receiver::<F128b, VandelmondeSolver<F128b>, _, _>::precomp_mt,
                receive_mt
            )
        }
        (SolverType::Paxos, MultiThreadOptimization::On) => {
            receiver_protocol!(
                ch_arcnize(receiver_channels),
                Arc::new(r_set),
                Receiver::<F128b, PaxosSolver<F128b>, _, _>::precomp_mt,
                receive_mt
            )
        }
    };

    let res: HashSet<F128b> = HashSet::from_iter(res);
    let intersection: HashSet<F128b> = HashSet::from_iter(intersection);

    assert_eq!(res, intersection);

    for handle in handles {
        handle.join().expect("Failed to join a thread.")?;
    }

    Ok(())
}

pub fn run(args: Args) -> Result<()> {
    let Args {
        num_parties,
        set_size,
        common_size,
        vole_type,
        solver_type,
        channel_type,
        port,
        multi_thread,
    } = args;

    let mut rng = AesRng::new();

    // create sets
    let (intersection, sets) = intersection_prepare(&mut rng, num_parties, set_size, common_size)
        .with_context(|| "Failed to prepare intersection.")?;

    println!("sets prepared.");

    // create channels
    let (receiver_channels, channels) = cli::create_channels(channel_type, num_parties, port)
        .with_context(|| "Failed to create channels.")?;

    println!("channels prepared.");

    // create vole share
    let (vole_share_for_s, vole_share_for_r) = match solver_type {
        SolverType::Vandelmonde => create_vole_sr::<VandelmondeSolver<F128b>>(vole_type, set_size),
        SolverType::Paxos => create_vole_sr::<PaxosSolver<F128b>>(vole_type, set_size),
    };

    println!("vole share prepared.");

    protocol_base(
        intersection,
        sets,
        receiver_channels,
        channels,
        multi_thread,
        solver_type,
        vole_share_for_s,
        vole_share_for_r,
    )?;

    Ok(())
}
