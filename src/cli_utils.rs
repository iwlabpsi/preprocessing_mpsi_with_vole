use crate::channel_utils::sync_channel::create_unix_channels;
use crate::channel_utils::sync_channel_by_cb::create_crossbeam_channels;
use crate::channel_utils::sync_channel_by_cb::{CrossbeamReceiver, CrossbeamSender};
use crate::channel_utils::tcp_channel::create_tcp_channels;
use crate::solver::{Solver, SolverParams};
use crate::vole::{
    LPNVoleReceiver, LPNVoleSender, OtVoleReceiver, OtVoleSender, VoleShareForReceiver,
    VoleShareForSender, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_MEDIUM, LPN_SETUP_SMALL,
};
use anyhow::Result;
use clap::{Parser, ValueEnum};
use ocelot::ot::{AlszReceiver as OtReceiver, AlszSender as OtSender};
use scuttlebutt::field::F128b;
use scuttlebutt::{AbstractChannel, SyncChannel};
use std::fmt::Display;
use std::net::TcpStream;
use std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixStream,
};

#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum VoleType {
    Ot,
    Lpn,
}

impl Display for VoleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoleType::Ot => write!(f, "ot"),
            VoleType::Lpn => write!(f, "lpn"),
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum SolverType {
    Vandelmonde,
    Paxos,
}

impl Display for SolverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolverType::Vandelmonde => write!(f, "vandelmonde"),
            SolverType::Paxos => write!(f, "paxos"),
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum ChannelType {
    Unix,
    Tcp,
    CrossBeam,
}

impl Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelType::Unix => write!(f, "unix"),
            ChannelType::Tcp => write!(f, "tcp"),
            ChannelType::CrossBeam => write!(f, "crossbeam"),
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum MultiThreadOptimization {
    On,
    Off,
}

impl Display for MultiThreadOptimization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiThreadOptimization::On => write!(f, "on"),
            MultiThreadOptimization::Off => write!(f, "off"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct PrePSIArgs {
    /// Number of participants in the protocol.
    #[arg(short = 'N', long, default_value_t = 3)]
    pub num_parties: usize,

    /// Number of elements of the set that each participant has.
    #[arg(short = 'n', long, default_value_t = 10)]
    pub set_size: usize,

    /// The size of the aggregate product of the sets that each party has.
    #[arg(short = 'm', long, default_value_t = 5)]
    pub common_size: usize,

    /// VOLE Sharing Methods.
    ///
    /// lpn: Learning Parity with Noise assumption
    ///
    /// ot : Oblivious Transfer
    #[arg(short = 'v', long = "vole", default_value_t = VoleType::Lpn)]
    pub vole_type: VoleType,

    /// Solver Methods.
    ///
    /// vandelmonde: Vandelmonde matrix inversion
    ///
    /// paxos      : PaXoS (probe-and-XOR of strings)
    #[arg(short = 's', long = "solver", default_value_t = SolverType::Paxos)]
    pub solver_type: SolverType,

    /// Channel Types.
    ///
    /// unix     : Unix domain socket
    ///
    /// tcp      : TCP socket
    ///
    /// crossbeam: Native channel of Rust
    #[arg(short = 'c', long = "channel", default_value_t = ChannelType::Unix)]
    pub channel_type: ChannelType,

    /// Port number for TCP channel.
    #[arg(short = 'p', long = "port", default_value_t = 10000)]
    pub port: usize,

    /// Multi-thread optimization.
    ///
    /// Off doesn't mean single-threaded and at least as many threads are created as parties.
    #[arg(short = 't', long = "threads", default_value_t = MultiThreadOptimization::On)]
    pub multi_thread: MultiThreadOptimization,

    /// Verbose mode. If specified, print the sets and the intersection.
    #[arg(long = "verbose", default_value_t = false)]
    pub verbose: bool,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct KmprtArgs {
    /// Number of participants in the protocol.
    #[arg(short = 'N', long, default_value_t = 3)]
    pub num_parties: usize,

    /// Number of elements of the set that each participant has.
    #[arg(short = 'n', long, default_value_t = 10)]
    pub set_size: usize,

    /// The size of the aggregate product of the sets that each party has.
    #[arg(short = 'm', long, default_value_t = 5)]
    pub common_size: usize,

    /// Channel Types.
    ///
    /// unix     : Unix domain socket
    ///
    /// tcp      : TCP socket
    ///
    /// crossbeam: Native channel of Rust
    #[arg(short = 'c', long = "channel", default_value_t = ChannelType::Unix)]
    pub channel_type: ChannelType,

    /// Port number for TCP channel.
    #[arg(short = 'p', long = "port", default_value_t = 10000)]
    pub port: usize,

    /// Verbose mode. If specified, print the sets and the intersection.
    #[arg(long = "verbose", default_value_t = false)]
    pub verbose: bool,
}

pub enum ChannelUnion {
    Unix(SyncChannel<BufReader<UnixStream>, BufWriter<UnixStream>>),
    Tcp(SyncChannel<BufReader<TcpStream>, BufWriter<TcpStream>>),
    CrossBeam(SyncChannel<CrossbeamReceiver, CrossbeamSender>),
}

use ChannelUnion::*;

impl AbstractChannel for ChannelUnion {
    #[inline(always)]
    fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Unix(c) => c.write_bytes(bytes),
            Tcp(c) => c.write_bytes(bytes),
            CrossBeam(c) => c.write_bytes(bytes),
        }
    }

    #[inline(always)]
    fn read_bytes(&mut self, bytes: &mut [u8]) -> std::io::Result<()> {
        match self {
            Unix(c) => c.read_bytes(bytes),
            Tcp(c) => c.read_bytes(bytes),
            CrossBeam(c) => c.read_bytes(bytes),
        }
    }

    #[inline(always)]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Unix(c) => c.flush(),
            Tcp(c) => c.flush(),
            CrossBeam(c) => c.flush(),
        }
    }

    #[inline(always)]
    fn clone(&self) -> Self {
        match self {
            Unix(c) => Unix(c.clone()),
            Tcp(c) => Tcp(c.clone()),
            CrossBeam(c) => CrossBeam(c.clone()),
        }
    }
}

macro_rules! make_union_channel {
    ($c:expr, $t:path) => {{
        let (receiver_channels, channels) = $c;
        Ok((
            receiver_channels
                .into_iter()
                .map(|(i, c)| (i, $t(c)))
                .collect(),
            channels
                .into_iter()
                .map(|cs| cs.into_iter().map(|(i, c)| (i, $t(c))).collect())
                .collect(),
        ))
    }};
}

type UCU = (usize, ChannelUnion);

pub fn create_channels(
    type_: ChannelType,
    nparties: usize,
    port: usize,
) -> Result<(Vec<UCU>, Vec<Vec<UCU>>)> {
    match type_ {
        ChannelType::Unix => make_union_channel!(create_unix_channels(nparties)?, Unix),
        ChannelType::Tcp => make_union_channel!(create_tcp_channels(nparties, port)?, Tcp),
        ChannelType::CrossBeam => {
            make_union_channel!(create_crossbeam_channels(nparties), CrossBeam)
        }
    }
}

#[derive(Clone, Copy)]
pub enum VoleShareForReceiverUnion {
    Ot(OtVoleReceiver<F128b, 128, OtReceiver>),
    Lpn(LPNVoleReceiver<F128b>),
}

#[derive(Clone, Copy)]
pub enum VoleShareForSenderUnion {
    Ot(OtVoleSender<F128b, 128, OtSender>),
    Lpn(LPNVoleSender<F128b>),
}

impl VoleShareForReceiver<F128b> for VoleShareForReceiverUnion {
    fn receive<C: AbstractChannel, RNG: rand::CryptoRng + rand::Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(Vec<F128b>, Vec<F128b>)> {
        match self {
            VoleShareForReceiverUnion::Ot(v) => v.receive(channel, rng, m),
            VoleShareForReceiverUnion::Lpn(v) => v.receive(channel, rng, m),
        }
    }
}

impl VoleShareForSender<F128b> for VoleShareForSenderUnion {
    fn receive<C: AbstractChannel, RNG: rand::CryptoRng + rand::Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(F128b, Vec<F128b>)> {
        match self {
            VoleShareForSenderUnion::Ot(v) => v.receive(channel, rng, m),
            VoleShareForSenderUnion::Lpn(v) => v.receive(channel, rng, m),
        }
    }
}

fn create_lpn_vole_sr<S: Solver<F128b>>(
    set_size: usize,
) -> (LPNVoleSender<F128b>, LPNVoleReceiver<F128b>) {
    let m_size = S::calc_params(set_size).code_length();
    let (setup_param, extend_param) = if m_size < (1 << 17) {
        println!("Small parameters are used.");
        (LPN_SETUP_SMALL, LPN_EXTEND_SMALL)
    } else {
        println!("Medium parameters are used.");
        (LPN_SETUP_MEDIUM, LPN_EXTEND_MEDIUM)
    };
    (
        LPNVoleSender::new(setup_param, extend_param),
        LPNVoleReceiver::new(setup_param, extend_param),
    )
}

pub fn create_vole_sr<S: Solver<F128b>>(
    vole_type: VoleType,
    set_size: usize,
) -> (VoleShareForSenderUnion, VoleShareForReceiverUnion) {
    match vole_type {
        VoleType::Ot => (
            VoleShareForSenderUnion::Ot(OtVoleSender::new()),
            VoleShareForReceiverUnion::Ot(OtVoleReceiver::new()),
        ),
        VoleType::Lpn => {
            let (s, r) = create_lpn_vole_sr::<S>(set_size);
            (
                VoleShareForSenderUnion::Lpn(s),
                VoleShareForReceiverUnion::Lpn(r),
            )
        }
    }
}
