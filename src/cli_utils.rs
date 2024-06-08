//! CLI (CommandLine Interface) utilities for "Preprocessing MPSI" and "Kmprt".
//!
//! Here, you can know the options for the protocol through enum types and structs.
//! See other modules for the actual implementation of the protocol or details of what options mean.

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

/// How to share VOLE (a kind of corelated randomness). More details: [vole](crate::vole).
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum VoleType {
    /// Use Oblivious Transfer. See [OtVoleSender] or [OtVoleReceiver].
    Ot,
    /// Use Learning Parity with Noise assumption. See [LPNVoleSender] or [LPNVoleReceiver].
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

/// Solver methods.
/// Solver encodes points (one point consists of a member of set and corresponding value such that the hash of member)
/// to vector (of something like coefficients) and decodes vector to points. More details: [solver](crate::solver).
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum SolverType {
    /// Use polynomial interpolation to encode algorithm. See [VandelmondeSolver](crate::solver::VandelmondeSolver).
    Vandelmonde,
    /// Use PaXoS (Probe-and-XOR of Strings) to encode algorithm. See [PaxosSolver](crate::solver::PaxosSolver).
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

/// Channel types. Channels are used to communicate between parties. More details: [channel_utils](crate::channel_utils).
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum ChannelType {
    /// Unix domain socket. See [UnixStream].
    Unix,
    /// TCP socket. See [TcpStream].
    Tcp,
    /// Native channel of Rust. See [CrossbeamReceiver] and [CrossbeamSender].
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

/// Multi-thread optimization.
/// Off doesn’t mean single-threaded version. The difference between the optimized version and the not one is that in where parties exchange messages.
/// More details: [psi::Receiver](crate::preprocessed::psi::Receiver) and [psi::Sender](crate::preprocessed::psi::Sender). `*_mt` functions are used in the optimized version.
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum MultiThreadOptimization {
    /// On means that the optimized version is used.
    On,
    /// Off means that the not optimized version is used.
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

/// Arguments for Preprocessing MPSI protocol.
/// This struct implements [clap::Parser] to make that this binary has CommandLine Arguments.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None, next_line_help = true)]
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
    #[arg(short = 'v', long = "vole", default_value_t = VoleType::Lpn)]
    pub vole_type: VoleType,

    /// Solver Methods.
    #[arg(short = 's', long = "solver", default_value_t = SolverType::Paxos)]
    pub solver_type: SolverType,

    /// Channel Types.
    #[arg(short = 'c', long = "channel", default_value_t = ChannelType::Unix)]
    pub channel_type: ChannelType,

    /// Port number for TCP channel.
    ///
    /// The port is used internally. No function to communicate externally is implemented. Sorry.
    #[arg(short = 'p', long = "port", default_value_t = 10000)]
    pub port: usize,

    /// Multi-thread optimization.
    ///
    /// Off doesn't mean single-threaded and at least as many threads are created as parties.
    #[arg(short = 't', long = "threads", default_value_t = MultiThreadOptimization::On)]
    pub multi_thread: MultiThreadOptimization,

    /// Verbose mode.
    ///
    /// If specified, print the sets and the intersection.
    #[arg(long = "verbose", default_value_t = false)]
    pub verbose: bool,
}

/// Arguments for Kmprt protocol.
/// This struct implements [clap::Parser] to make that this binary has CommandLine Arguments.
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
    #[arg(short = 'c', long = "channel", default_value_t = ChannelType::Unix)]
    pub channel_type: ChannelType,

    /// Port number for TCP channel.
    ///
    /// The port is used internally. No function to communicate externally is implemented. Sorry.
    #[arg(short = 'p', long = "port", default_value_t = 10000)]
    pub port: usize,

    /// Verbose mode. If specified, print the sets and the intersection.
    #[arg(long = "verbose", default_value_t = false)]
    pub verbose: bool,
}

/// Enum type to handle multiple channel types on runtime. Please ignore it :)
pub enum ChannelUnion {
    /// Unix domain socket. See [UnixStream].
    Unix(SyncChannel<BufReader<UnixStream>, BufWriter<UnixStream>>),
    /// TCP socket. See [TcpStream].
    Tcp(SyncChannel<BufReader<TcpStream>, BufWriter<TcpStream>>),
    /// Native channel of Rust. See [CrossbeamReceiver] and [CrossbeamSender].
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

type Ucu = (usize, ChannelUnion);

/// Create channels for the protocol. Runtime utility.
pub fn create_channels(
    type_: ChannelType,
    nparties: usize,
    port: usize,
) -> Result<(Vec<Ucu>, Vec<Vec<Ucu>>)> {
    match type_ {
        ChannelType::Unix => make_union_channel!(create_unix_channels(nparties)?, Unix),
        ChannelType::Tcp => make_union_channel!(create_tcp_channels(nparties, port)?, Tcp),
        ChannelType::CrossBeam => {
            make_union_channel!(create_crossbeam_channels(nparties), CrossBeam)
        }
    }
}

/// Enum type to handle multiple vole share types for receivers on runtime. Please ignore it :)
#[derive(Clone, Copy)]
pub enum VoleShareForReceiverUnion {
    /// Use Oblivious Transfer. See [OtVoleReceiver].
    Ot(OtVoleReceiver<F128b, 128, OtReceiver>),
    /// Use Learning Parity with Noise assumption. See [LPNVoleReceiver].
    Lpn(LPNVoleReceiver<F128b>),
}

/// Enum type to handle multiple vole share types for senders on runtime. Please ignore it :)
#[derive(Clone, Copy)]
pub enum VoleShareForSenderUnion {
    /// Use Oblivious Transfer. See [OtVoleSender].
    Ot(OtVoleSender<F128b, 128, OtSender>),
    /// Use Learning Parity with Noise assumption. See [LPNVoleSender].
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

/// Create vole sender and receiver for the protocol. Runtime utility.
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
