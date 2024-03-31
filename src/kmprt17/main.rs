use clap::Parser;
use itertools::Itertools;
use preprocessing_mpsi_with_vole::cli_utils::{create_channels, KmprtArgs};
use preprocessing_mpsi_with_vole::kmprt17::{Receiver, Sender};
use rand::Rng;
use scuttlebutt::{AesRng, Block};

fn main() {
    let args = KmprtArgs::parse();

    protocol(args);
}

fn protocol(
    KmprtArgs {
        num_parties,
        set_size,
        common_size,
        channel_type,
        port,
        verbose,
    }: KmprtArgs,
) {
    let mut rng = AesRng::new();

    if common_size > set_size {
        panic!("common_size > set_size");
    }

    let intersection = (0..common_size).map(|_| rng.gen::<Block>()).collect_vec();

    let sets = (0..num_parties)
        .map(|i| {
            let mut set = intersection.clone();
            set.extend((common_size..set_size).map(|_| rng.gen::<Block>()));

            if verbose {
                println!("sets[{}] = {:?}", i, set);
            }

            set
        })
        .collect_vec();

    // create channels
    let (mut receiver_channels, channels) =
        create_channels(channel_type, num_parties, port).unwrap();

    for (i, mut channels) in channels.into_iter().enumerate() {
        // create and fork senders
        let pid = i + 1;
        let my_set = sets[pid].clone();
        std::thread::spawn(move || {
            let mut rng = AesRng::new();
            let mut sender = Sender::init(pid, &mut channels, &mut rng).unwrap();
            sender.send(&my_set, &mut channels, &mut rng).unwrap();
        });
    }

    // create and run receiver
    let mut receiver = Receiver::init(&mut receiver_channels, &mut rng).unwrap();
    let res = receiver
        .receive(&sets[0], &mut receiver_channels, &mut rng)
        .unwrap();

    if verbose {
        println!("intersection = {:?}", res);
    }

    assert_eq!(res, intersection);
}
