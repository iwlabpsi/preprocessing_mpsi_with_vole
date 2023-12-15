use crossbeam::channel::{unbounded, Receiver, RecvTimeoutError, SendError, Sender};
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::time::Duration;

pub struct CrossbeamSender(Sender<u8>);
pub struct CrossbeamReceiver<const D: u64>(Receiver<u8>);

impl Write for CrossbeamSender {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        for &v in buf {
            if let Err(SendError(v)) = self.0.send(v) {
                return Err(Error::new(ErrorKind::BrokenPipe, SendError(v)));
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<const D: u64> Read for CrossbeamReceiver<D> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        for i in 0..buf.len() {
            match self.0.recv_timeout(Duration::from_millis(D)) {
                Ok(v) => buf[i] = v,
                Err(RecvTimeoutError::Timeout) => return Ok(i),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::new(
                        ErrorKind::BrokenPipe,
                        RecvTimeoutError::Disconnected,
                    ))
                }
            }
        }

        Ok(buf.len())
    }
}

pub fn cbch_pair<const D: u64>() -> (CrossbeamSender, CrossbeamReceiver<D>) {
    let (s, r) = unbounded();
    (CrossbeamSender(s), CrossbeamReceiver::<D>(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scuttlebutt::{AbstractChannel, SyncChannel};

    const TIMEOUT: u64 = 100;

    #[test]
    fn test() {
        let (mut s1, mut r1) = cbch_pair::<TIMEOUT>();

        let handle = std::thread::spawn(move || {
            let mut v = vec![0u8; 3];
            r1.read_exact(&mut v).unwrap();
            assert_eq!(v, [1, 2, 3]);
        });

        let v = [1, 2, 3];
        s1.write_all(&v).unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn test_empty_res() {
        let (mut s1, mut r1) = cbch_pair::<TIMEOUT>();

        let handle = std::thread::spawn(move || {
            let mut v = vec![0u8; 16];
            let mut count = 0;
            while let Ok(s) = r1.read(&mut v) {
                if s == 0 {
                    break;
                }

                count += s;
            }

            assert_eq!(count, 3);
        });

        let v = [1, 2, 3];
        s1.write_all(&v).unwrap();

        handle.join().unwrap();
    }

    #[test]
    fn test_broken_pipe() {
        let (mut s1, mut r1) = cbch_pair::<TIMEOUT>();

        let handle = std::thread::spawn(move || {
            let mut v = vec![0u8; 3];
            r1.read_exact(&mut v).unwrap();
            assert_eq!(v, [1, 2, 3]);

            let mut v = vec![0u8; 3];
            let e = r1.read_exact(&mut v).unwrap_err();
            assert_eq!(e.kind(), ErrorKind::BrokenPipe);
        });

        let v = [1, 2, 3];
        s1.write_all(&v).unwrap();
        drop(s1);

        handle.join().unwrap();
    }

    #[test]
    fn test_channel() {
        let (s1, r1) = cbch_pair::<TIMEOUT>();
        let (s2, r2) = cbch_pair::<TIMEOUT>();
        let mut ch1 = SyncChannel::new(r1, s2);
        let mut ch2 = SyncChannel::new(r2, s1);

        let handle = std::thread::spawn(move || {
            let n = ch1.read_u32().unwrap();
            assert_eq!(n, 123);
            ch1.write_u32(n * 2).unwrap();
        });

        let n = 123;
        ch2.write_u32(n).unwrap();
        let n = ch2.read_u32().unwrap();
        assert_eq!(n, 246);

        handle.join().unwrap();
    }
}
