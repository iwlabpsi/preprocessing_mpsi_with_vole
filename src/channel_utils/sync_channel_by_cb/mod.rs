use itertools::Itertools;
use scuttlebutt::SyncChannel;
pub mod crossbeam_wrapper;
pub use crossbeam_wrapper::{cbch_pair, CrossbeamReceiver, CrossbeamSender};

type Channel<const D: u64> = (usize, SyncChannel<CrossbeamReceiver<D>, CrossbeamSender>);

pub fn create_crossbeam_channels<const D: u64>(
    nparties: usize,
) -> (Vec<Channel<D>>, Vec<Vec<Channel<D>>>) {
    let mut channels = (0..nparties)
        .map(|_| (0..nparties).map(|_| None).collect_vec())
        .collect_vec();

    for i in 0..nparties {
        for j in 0..nparties {
            if i != j {
                let (sr, rl) = cbch_pair::<D>();
                let (sl, rr) = cbch_pair::<D>();
                let left = SyncChannel::new(rl, sl);
                let right = SyncChannel::new(rr, sr);
                channels[i][j] = Some((j, left));
                channels[j][i] = Some((i, right));
            }
        }
    }

    let mut channels = channels
        .into_iter()
        .map(|cs| cs.into_iter().flatten().collect_vec())
        .collect_vec();

    let receiver_channels = channels.remove(0);

    (receiver_channels, channels)
}
