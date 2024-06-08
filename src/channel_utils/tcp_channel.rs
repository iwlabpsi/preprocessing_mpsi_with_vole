//! Module about tcp channel. See [TcpStream].
//! This module provides a function to create a set of tcp stream channels for receiver and senders.

use anyhow::{bail, Context, Result};
use scuttlebutt::SyncChannel;
use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::sleep;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

type Channel = (
    usize,
    SyncChannel<BufReader<TcpStream>, BufWriter<TcpStream>>,
);

fn create_tcp_channel_for_party(
    nparties: usize,
    base_port: usize,
    me: usize,
) -> Result<Vec<Channel>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], (base_port + me) as _));
    let listener = TcpListener::bind(addr)
        .with_context(|| format!("me={} addr={} @{}:{}", me, addr, file!(), line!()))?;

    sleep(Duration::from_millis(100 * me as u64));

    let mut streams = (0..me)
        .map(|i| {
            let port = base_port + i;
            let addr = SocketAddr::from(([127, 0, 0, 1], port as _));
            let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)
                .with_context(|| format!("me={} addr={} @{}:{}", me, addr, file!(), line!()))?;
            let m = me.to_be_bytes();
            stream
                .write(&m)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            let mut buf = [0u8; 8];
            stream
                .read(&mut buf)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            let m = usize::from_be_bytes(buf);
            Ok((m, stream))
        })
        .collect::<Result<Vec<(usize, TcpStream)>>>()?;

    let recv_streams = listener
        .incoming()
        .take(nparties - 1 - me)
        .map(|s| {
            let mut s = s.with_context(|| format!("@{}:{}", file!(), line!()))?;

            let mut buf = [0u8; 8];
            s.read(&mut buf)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            let m = usize::from_be_bytes(buf);
            let mm = me.to_be_bytes();
            s.write(&mm)
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            Ok((m, s))
        })
        .collect::<Result<Vec<_>>>()?;

    streams.extend(recv_streams);

    streams.sort_by(|a, b| a.0.cmp(&b.0));

    let res = streams
        .into_iter()
        .map(|(m, s)| {
            let ss = s
                .try_clone()
                .with_context(|| format!("@{}:{}", file!(), line!()))?;
            Ok((m, SyncChannel::new(BufReader::new(ss), BufWriter::new(s))))
        })
        .collect::<Result<Vec<Channel>>>()?;

    Ok(res)
}

/// Return a vector of channels for sender channel.
pub fn create_tcp_channels_for_sender(
    nparties: usize,
    port: usize,
    me: usize,
) -> Result<Vec<Channel>> {
    if me == 0 {
        bail!("me must be > 0 (now me = {})", 0);
    }

    let res = create_tcp_channel_for_party(nparties, port, me)?;

    Ok(res)
}

/// Return a vector of channels for receiver channel.
pub fn create_tcp_channels_for_receiver(nparties: usize, port: usize) -> Result<Vec<Channel>> {
    let res = create_tcp_channel_for_party(nparties, port, 0)?;

    Ok(res)
}

/// Create a set of tcp stream socket channels. See [TcpStream].
///
/// Return a tuple of two vectors of channels. The first vector contains the receiver channels, and the second vector contains the sender channels.
pub fn create_tcp_channels(
    nparties: usize,
    port: usize,
) -> Result<(Vec<Channel>, Vec<Vec<Channel>>)> {
    let receiver_handle =
        std::thread::spawn(move || create_tcp_channels_for_receiver(nparties, port));

    let handles = (1..nparties)
        .map(|me| std::thread::spawn(move || create_tcp_channel_for_party(nparties, port, me)))
        .collect::<Vec<_>>();

    let receiver_channels = receiver_handle.join().unwrap()?;
    let channels = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Result<Vec<Vec<Channel>>>>()?;

    Ok((receiver_channels, channels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::set_utils::create_sets_random;
    use popsicle::kmprt::{Receiver as KmprtReceiver, Sender as KmprtSender};
    use scuttlebutt::{AbstractChannel, AesRng, Block};

    #[test]
    fn test_2party() {
        let nparties = 2;

        let handle = std::thread::spawn(move || {
            let mut channels = create_tcp_channels_for_sender(nparties, 10000, 1).unwrap();
            let channel = &mut channels[0].1;

            let m = channel.read_usize().unwrap();
            assert_eq!(m, 1);

            channel.write_usize(0).unwrap();
        });

        let mut channels = create_tcp_channels_for_receiver(nparties, 10000).unwrap();

        let channel = &mut channels[0].1;

        channel.write_usize(1).unwrap();

        let m = channel.read_usize().unwrap();
        assert_eq!(m, 0);

        handle.join().unwrap();
    }

    fn test_nparty(nparties: usize, base_port: usize) {
        let handles = (1..nparties)
            .map(|me| {
                std::thread::spawn(move || {
                    let mut channels =
                        create_tcp_channels_for_sender(nparties, base_port, me).unwrap();

                    for (i, c) in channels.iter_mut() {
                        let i = *i;
                        if i < me {
                            c.write_usize(me).unwrap();
                            let m = c.read_usize().unwrap();
                            assert_eq!(m, i);
                        } else if i > me {
                            let m = c.read_usize().unwrap();
                            assert_eq!(m, i);
                            c.write_usize(me).unwrap();
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        let mut channels = create_tcp_channels_for_receiver(nparties, base_port).unwrap();

        for (i, c) in channels.iter_mut() {
            let i = *i;
            let m = c.read_usize().unwrap();
            assert_eq!(m, i);
            c.write_usize(0).unwrap();
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_3party() {
        test_nparty(3, 5000);
    }

    #[test]
    fn test_4party() {
        test_nparty(4, 15000);
    }

    #[test]
    fn test_5party() {
        test_nparty(5, 20000);
    }

    fn test_nparty_psi(nparties: usize, base_port: usize) {
        let mut rng = AesRng::new();
        let (_common, sets): (Vec<Block>, _) = create_sets_random(nparties, 32, &mut rng).unwrap();

        let handles = (1..nparties)
            .map(|me| {
                let set = sets[me].clone();
                std::thread::spawn(move || {
                    let mut channels =
                        create_tcp_channels_for_sender(nparties, base_port, me).unwrap();
                    let mut rng = AesRng::new();

                    let mut sender = KmprtSender::init(me, &mut channels, &mut rng).unwrap();
                    sender.send(&set, &mut channels, &mut rng).unwrap();
                })
            })
            .collect::<Vec<_>>();

        let mut channels = create_tcp_channels_for_receiver(nparties, base_port).unwrap();
        let recv_set = sets[0].clone();

        let mut receiver = KmprtReceiver::init(&mut channels, &mut rng).unwrap();
        let res = receiver
            .receive(&recv_set, &mut channels, &mut rng)
            .unwrap();

        for h in handles {
            h.join().unwrap();
        }

        for x in res.iter() {
            assert!(sets.iter().all(|s| s.contains(x)));
        }
    }

    #[test]
    fn test_3party_psi() {
        test_nparty_psi(3, 5050);
    }

    #[test]
    fn test_4party_psi() {
        test_nparty_psi(4, 15050);
    }

    #[test]
    fn test_5party_psi() {
        test_nparty_psi(5, 20050);
    }
}
