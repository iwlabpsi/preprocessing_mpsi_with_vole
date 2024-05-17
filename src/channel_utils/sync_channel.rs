//! Module about unix domain socket channel. See [UnixStream].
//! This module provides a function to create a set of unix domain socket channels for receiver and senders.

use anyhow::{Context, Result};
use itertools::Itertools;
use scuttlebutt::SyncChannel;
use std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixStream,
};

type Channel = (
    usize,
    SyncChannel<BufReader<UnixStream>, BufWriter<UnixStream>>,
);

/// Create a set of unix domain socket channels. See [UnixStream].
///
/// Return a tuple of two vectors of channels. The first vector contains the receiver channels, and the second vector contains the sender channels.
pub fn create_unix_channels(nparties: usize) -> Result<(Vec<Channel>, Vec<Vec<Channel>>)> {
    let mut channels = (0..nparties)
        .map(|_| (0..nparties).map(|_| None).collect_vec())
        .collect_vec();

    for i in 0..nparties {
        for j in 0..nparties {
            if i != j {
                let (s, r) =
                    UnixStream::pair().with_context(|| format!("@{}:{}", file!(), line!()))?;
                let rs = s
                    .try_clone()
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
                let rr = r
                    .try_clone()
                    .with_context(|| format!("@{}:{}", file!(), line!()))?;
                let left = SyncChannel::new(BufReader::new(rs), BufWriter::new(s));
                let right = SyncChannel::new(BufReader::new(rr), BufWriter::new(r));
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

    Ok((receiver_channels, channels))
}
