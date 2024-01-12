use anyhow::Error;
pub use ocelot::svole::wykw::{
    LpnParams, LPN_EXTEND_LARGE, LPN_EXTEND_MEDIUM, LPN_EXTEND_SMALL, LPN_SETUP_LARGE,
    LPN_SETUP_MEDIUM, LPN_SETUP_SMALL,
};
use rand::{CryptoRng, Rng};
use scuttlebutt::channel::AbstractChannel;
use scuttlebutt::field::FiniteField as FF;

pub mod lpn_based;
pub use lpn_based::{LPNVoleReceiver, LPNVoleSender};
pub mod ot_based;
pub use ot_based::{OtVoleReceiver, OtVoleSender};

pub trait VoleShareForSender<F: FF>: Clone + Copy {
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(F, Vec<F>), Error>;
}

pub trait VoleShareForReceiver<F: FF>: Clone + Copy {
    fn receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        &mut self,
        channel: &mut C,
        rng: &mut RNG,
        m: usize,
    ) -> Result<(Vec<F>, Vec<F>), Error>;
}
