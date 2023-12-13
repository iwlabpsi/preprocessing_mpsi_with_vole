use anyhow::{Context, Result};
use scuttlebutt::field::FiniteField as FF;
use scuttlebutt::AbstractChannel;
use std::sync::{Arc, Mutex};
use typenum::marker_traits::Unsigned;

pub mod sync_channel;
pub mod tcp_channel;

pub fn write_vec_f<F, C>(channel: &mut C, v: &[F]) -> Result<usize>
where
    F: FF,
    C: AbstractChannel,
{
    let bytes = v
        .iter()
        .flat_map(|x| x.to_bytes().to_vec())
        .collect::<Vec<_>>();

    let len = bytes.len();

    channel
        .write_usize(len)
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

    channel
        .write_bytes(&bytes)
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

    channel
        .flush()
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

    Ok(len)
}

pub fn read_vec_f<F, C>(channel: &mut C) -> Result<Vec<F>>
where
    F: FF,
    C: AbstractChannel,
{
    let bytes_len = channel
        .read_usize()
        .with_context(|| format!("@{}:{}", file!(), line!()))?;

    let mut res = vec![0u8; bytes_len];

    channel.read_bytes(&mut res)?;

    let res = res
        .chunks(F::ByteReprLen::to_usize())
        .into_iter()
        .map(|x| {
            F::from_bytes(x.as_ref().into()).with_context(|| format!("@{}:{}", file!(), line!()))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(res)
}

pub fn ch_arcnize<C>(
    receiver_channels: Vec<(usize, C)>,
    channels: Vec<Vec<(usize, C)>>,
) -> Result<(
    Vec<(usize, Arc<Mutex<C>>)>,
    Vec<Vec<(usize, Arc<Mutex<C>>)>>,
)>
where
    C: AbstractChannel,
{
    let receiver_channels = receiver_channels
        .into_iter()
        .map(|(i, c)| (i, Arc::new(Mutex::new(c))))
        .collect::<Vec<_>>();
    let channels = channels
        .into_iter()
        .map(|cs| {
            cs.into_iter()
                .map(|(i, c)| (i, Arc::new(Mutex::new(c))))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    Ok((receiver_channels, channels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use scuttlebutt::{field::F128b, AesRng, Channel};
    use std::io::{BufReader, BufWriter};
    use std::os::unix::net::UnixStream;

    #[test]
    fn test_write_read_vec_f() {
        let mut rng = AesRng::new();

        let v = (0..10).map(|_| rng.gen::<F128b>()).collect::<Vec<_>>();
        let w = v.clone();

        let (sender, receiver) = UnixStream::pair().unwrap();
        let handle = std::thread::spawn(move || {
            let mut channel = Channel::new(
                BufReader::new(sender.try_clone().unwrap()),
                BufWriter::new(sender),
            );

            channel.write_bytes(b"hello").unwrap();
            channel.flush().unwrap();

            let _len = write_vec_f(&mut channel, &w).unwrap();
        });

        let mut channel = Channel::new(
            BufReader::new(receiver.try_clone().unwrap()),
            BufWriter::new(receiver),
        );

        let mut buf = [0u8; 5];
        channel.read_bytes(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");

        let res = read_vec_f::<F128b, _>(&mut channel).unwrap();

        handle.join().unwrap();

        assert_eq!(v, res);
    }
}
