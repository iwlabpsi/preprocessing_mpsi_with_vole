use anyhow::{Context, Result};
use itertools::Itertools;
use scuttlebutt::SyncChannel;
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixStream,
};
/*
use criterion::{
    measurement::{Measurement, ValueFormatter},
    Throughput,
};
*/

// You should use scuttlebutt::channel::TrackChannel to measure the traffic.
// Unfortunately, I didn't notice its existence until I reinvented the wheel.

#[derive(Debug)]
pub(crate) struct TrafficBytes {
    trd: Option<thread::JoinHandle<usize>>,
}

impl TrafficBytes {
    pub fn new() -> (Self, Sender<usize>) {
        let (tmp_tx, tmp_rx): (Sender<Sender<usize>>, Receiver<Sender<usize>>) = channel();

        let handle = thread::spawn(move || {
            let (tx, rx): (Sender<usize>, Receiver<usize>) = channel();
            tmp_tx.send(tx).unwrap();

            let mut total_bytes = 0;
            while let Ok(bytes) = rx.recv() {
                total_bytes += bytes;
            }
            total_bytes
        });

        let tx = tmp_rx.recv().unwrap();

        let slf = Self { trd: Some(handle) };

        (slf, tx)
    }

    pub fn total_bytes(&mut self) -> usize {
        let Some(handle) = self.trd.take() else {
            panic!("total_bytes() called twice");
        };
        handle.join().unwrap()
    }
}

pub(crate) struct ByteCountRead<R: Read> {
    inner: R,
    tx: Sender<usize>,
}

impl<R: Read> ByteCountRead<R> {
    pub fn new(inner: R, tx: Sender<usize>) -> Self {
        Self { inner, tx }
    }
}

impl<R: Read> Read for ByteCountRead<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let res = self.inner.read(buf);
        if let Ok(n) = res {
            self.tx.send(n).unwrap();
        }
        res
    }
}

pub(crate) struct ByteCountWrite<W: Write> {
    inner: W,
    tx: Sender<usize>,
}

impl<W: Write> ByteCountWrite<W> {
    pub fn new(inner: W, tx: Sender<usize>) -> Self {
        Self { inner, tx }
    }
}

impl<W: Write> Write for ByteCountWrite<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = self.inner.write(buf);
        if let Ok(n) = res {
            self.tx.send(n).unwrap();
        }
        res
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

type ReadCountChannel = (
    usize,
    SyncChannel<ByteCountRead<BufReader<UnixStream>>, BufWriter<UnixStream>>,
);

pub(crate) fn create_readcount_channels(
    nparties: usize,
    tx: Sender<usize>,
) -> Result<(Vec<ReadCountChannel>, Vec<Vec<ReadCountChannel>>)> {
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
                let left_tx = tx.clone();
                let right_tx = tx.clone();
                let left = SyncChannel::new(
                    ByteCountRead::new(BufReader::new(rs), left_tx),
                    BufWriter::new(s),
                );
                let right = SyncChannel::new(
                    ByteCountRead::new(BufReader::new(rr), right_tx),
                    BufWriter::new(r),
                );
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

type WriteCountChannel = (
    usize,
    SyncChannel<BufReader<UnixStream>, ByteCountWrite<BufWriter<UnixStream>>>,
);

pub(crate) fn create_writecount_channels(
    nparties: usize,
    tx: Sender<usize>,
) -> Result<(Vec<WriteCountChannel>, Vec<Vec<WriteCountChannel>>)> {
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
                let left_tx = tx.clone();
                let right_tx = tx.clone();
                let left = SyncChannel::new(
                    BufReader::new(rs),
                    ByteCountWrite::new(BufWriter::new(s), left_tx),
                );
                let right = SyncChannel::new(
                    BufReader::new(rr),
                    ByteCountWrite::new(BufWriter::new(r), right_tx),
                );
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

/*
pub(crate) struct TrafficBytesFormatter;

impl TrafficBytesFormatter {
    // tbytes = traffic bytes.
    // so, typical = 1 byte

    fn bytes_per_traffic_bytes(
        &self,
        bytes: f64,
        typical: f64,
        values: &mut [f64],
    ) -> &'static str {
        let bytes_per_tbytes = bytes / typical;
        let (denominator, unit) = if bytes_per_tbytes < 1024.0 {
            (1.0, "  B/B")
        } else if bytes_per_tbytes < 1024.0 * 1024.0 {
            (1024.0, "KiB/B")
        } else if bytes_per_tbytes < 1024.0 * 1024.0 * 1024.0 {
            (1024.0 * 1024.0, "MiB/B")
        } else {
            (1024.0 * 1024.0 * 1024.0, "GiB/B")
        };

        for val in values {
            let bytes_per_tbytes = bytes / (*val);
            *val = bytes_per_tbytes / denominator;
        }

        unit
    }

    fn bytes_per_tbytes_decimal(
        &self,
        bytes: f64,
        typical: f64,
        values: &mut [f64],
    ) -> &'static str {
        let bytes_per_tbytes = bytes / typical;
        let (denominator, unit) = if bytes_per_tbytes < 1000.0 {
            (1.0, " B/B")
        } else if bytes_per_tbytes < 1000.0 * 1000.0 {
            (1000.0, "KB/B")
        } else if bytes_per_tbytes < 1000.0 * 1000.0 * 1000.0 {
            (1000.0 * 1000.0, "MB/B")
        } else {
            (1000.0 * 1000.0 * 1000.0, "GB/B")
        };

        for val in values {
            let bytes_per_tbytes = bytes / (*val);
            *val = bytes_per_tbytes / denominator;
        }

        unit
    }

    fn elements_per_tbytes(&self, elems: f64, typical: f64, values: &mut [f64]) -> &'static str {
        let elems_per_tbytes = elems * typical;
        let (denominator, unit) = if elems_per_tbytes < 1000.0 {
            (1.0, " /B")
        } else if elems_per_tbytes < 1000.0 * 1000.0 {
            (1000.0, "K/B")
        } else if elems_per_tbytes < 1000.0 * 1000.0 * 1000.0 {
            (1000.0 * 1000.0, "M/B")
        } else {
            (1000.0 * 1000.0 * 1000.0, "G/B")
        };

        for val in values {
            let elems_per_tbytes = elems / (*val);
            *val = elems_per_tbytes / denominator;
        }

        unit
    }
}

impl ValueFormatter for TrafficBytesFormatter {
    fn scale_throughputs(
        &self,
        typical: f64,
        throughput: &Throughput,
        values: &mut [f64],
    ) -> &'static str {
        match *throughput {
            Throughput::Bytes(bytes) => self.bytes_per_traffic_bytes(bytes as f64, typical, values),
            Throughput::BytesDecimal(bytes) => {
                self.bytes_per_tbytes_decimal(bytes as f64, typical, values)
            }
            Throughput::Elements(elems) => self.elements_per_tbytes(elems as f64, typical, values),
        }
    }

    fn scale_values(&self, typical_value: f64, values: &mut [f64]) -> &'static str {
        let (denominator, unit) = if typical_value < 1024.0 {
            (1.0, "  B")
        } else if typical_value < 1024.0 * 1024.0 {
            (1024.0, "KiB")
        } else if typical_value < 1024.0 * 1024.0 * 1024.0 {
            (1024.0 * 1024.0, "MiB")
        } else {
            (1024.0 * 1024.0 * 1024.0, "GiB")
        };

        for val in values {
            *val /= denominator;
        }

        unit
    }

    fn scale_for_machines(&self, _values: &mut [f64]) -> &'static str {
        "B"
    }
}

pub(crate) struct TrafficBytesMeasurement;
impl Measurement for TrafficBytesMeasurement {
    type Intermediate = ();
    type Value = usize;

    fn start(&self) -> Self::Intermediate {
        ()
    }
    fn end(&self, mut _i: Self::Intermediate) -> Self::Value {
        0
    }
    fn add(&self, v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        *v1 + *v2
    }
    fn zero(&self) -> Self::Value {
        0
    }
    fn to_f64(&self, val: &Self::Value) -> f64 {
        *val as f64
    }
    fn formatter(&self) -> &dyn ValueFormatter {
        &TrafficBytesFormatter
    }
}

/*
pub(crate) struct TrafficBytesMeasurement {
    sender_tx: Sender<Sender<usize>>,
}

impl TrafficBytesMeasurement {
    pub fn new() -> (Self, Receiver<Sender<usize>>) {
        let (sender_tx, sender_rx) = channel();
        (Self { sender_tx }, sender_rx)
    }
}

impl Measurement for TrafficBytesMeasurement {
    type Intermediate = TrafficBytes;
    type Value = usize;

    fn start(&self) -> Self::Intermediate {
        let (traffic_bytes, tx) = TrafficBytes::new();
        self.sender_tx.send(tx).unwrap();
        traffic_bytes
    }
    fn end(&self, mut i: Self::Intermediate) -> Self::Value {
        i.total_bytes()
    }
    fn add(&self, v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        *v1 + *v2
    }
    fn zero(&self) -> Self::Value {
        0
    }
    fn to_f64(&self, val: &Self::Value) -> f64 {
        *val as f64
    }
    fn formatter(&self) -> &dyn ValueFormatter {
        &TrafficBytesFormatter
    }
}
*/

*/
