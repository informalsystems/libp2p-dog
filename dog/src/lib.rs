mod config;
mod layer;
pub mod protocol;

mod proto {
    #![allow(unreachable_pub)]
    include!("generated/mod.rs");
    pub(crate) use self::dog::pb::{Tx, RPC};
}

pub use self::{
    layer::{Behaviour, DogEvent},
    protocol::{DogRpc, DogTransaction},
};
