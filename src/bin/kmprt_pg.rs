use itertools::Itertools;
use popsicle::kmprt::{Receiver, Sender};
use preprocessing_mpsi_with_vole::channel_utils::sync_channel::create_unix_channels;
use rand::Rng;
use scuttlebutt::{AesRng, Block};

fn main() {
    protocol(5, 5, 2);
}

fn protocol(nparties: usize, set_size: usize, intersection_size: usize) {
    let mut rng = AesRng::new();

    if intersection_size > set_size {
        panic!("intersection_size > set_size");
    }

    let intersection = (0..intersection_size)
        .map(|_| rng.gen::<Block>())
        .collect_vec();

    let sets = (0..nparties)
        .map(|i| {
            let mut set = intersection.clone();
            set.extend((intersection_size..set_size).map(|_| rng.gen::<Block>()));

            println!("sets[{}] = {:?}", i, set);

            set
        })
        .collect_vec();

    // create channels
    let (mut receiver_channels, channels) = create_unix_channels(nparties).unwrap();

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

    println!("intersection = {:?}", res);

    assert_eq!(res, intersection);
}
