use aes::*;
// use block_modes::{block_padding::NoPadding, BlockMode, Ecb};
use ethereum_types::{H128, H256};
use generic_array::{typenum::U16, GenericArray};
use sha3::{Digest, Keccak256};

pub type HeaderBytes = GenericArray<u8, U16>;

#[derive(Debug)]
pub struct MAC {
    secret: H256,
    hasher: Keccak256,
}

impl MAC {
    pub fn new(secret: H256) -> Self {
        Self {
            secret,
            hasher: Keccak256::new(),
        }
    }

     pub fn update(&mut self, data: &[u8]) {
         self.hasher.update(data);
     }

    // pub fn update_header(&mut self, data: &HeaderBytes) {
    //     let aes = Ecb::<_, NoPadding>::new(Aes256::new_from_slice)
    // }

    pub fn digest(&self) -> H128 {
        H128::from_slice(&self.hasher.clone().finalize()[0..16])
    }
}
