//! Module about native channel of Rust. See [crossbeam].
//! This module provides a function to create a set of crossbeam channels for a given number of parties (receiver and senders).

use itertools::Itertools;
use scuttlebutt::SyncChannel;
pub mod crossbeam_wrapper;
use crossbeam_wrapper::cbch_pair;
pub use crossbeam_wrapper::{CrossbeamReceiver, CrossbeamSender};

type Channel = (usize, SyncChannel<CrossbeamReceiver, CrossbeamSender>);

/// Create a set of crossbeam channels.
///
/// Return a tuple of two vectors of channels. The first vector contains the receiver channels, and the second vector contains the sender channels.
pub fn create_crossbeam_channels(nparties: usize) -> (Vec<Channel>, Vec<Vec<Channel>>) {
    let mut channels = (0..nparties)
        .map(|_| (0..nparties).map(|_| None).collect_vec())
        .collect_vec();

    for i in 0..nparties {
        for j in 0..nparties {
            if i != j {
                let (sr, rl) = cbch_pair();
                let (sl, rr) = cbch_pair();
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
