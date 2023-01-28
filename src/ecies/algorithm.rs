use crate::types::PeerId;
use educe::Educe;
use ethereum_types::H256;
use secp256k1::{PublicKey, SecretKey};
use sha2::{digest::Digest, Sha256};

const PROTOCOL_VERSION: usize = 4;

fn ecdh_x(public_key: &PublicKey, secret_key: &SecretKey) -> H256 {
    let shared_secret = secp256k1::ecdh::SharedSecret::new(public_key, secret_key);
    H256::from_slice(&shared_secret.secret_bytes())
}

fn kdf(secret: H256, s1: &[u8], dest: &mut [u8]) {
    let mut ctr = 1_u32;
    let mut written = 0_usize;
    while written < dest.len() {
        let mut hasher = Sha256::default();
        let ctrs = [
            (ctr >> 24) as u8,
            (ctr >> 16) as u8,
            (ctr >> 8) as u8,
            ctr as u8,
        ];
        hasher.update(&ctrs);
        hasher.update(secret.as_bytes());
        hasher.update(s1);
        let d = hasher.finalize();
        dest[written..(written + 32)].copy_from_slice(&d);
        written += 32;
        ctr += 1;
    }
}

#[derive(Educe)]
#[educe(Debug)]
pub struct ECIES {
    #[educe(Debug(ignore))]
    secret_key: SecretKey,
    public_key: PublicKey,
    remote_public_key: Option<PublicKey>,

    pub(crate) remote_id: Option<PeerId>,

    #[educe(Debug(ignore))]
    ephemeral_secret_key: SecretKey,
    ephemeral_public_key: PublicKey,
    ephemeral_shared_secret: Option<H256>,
    remote_ephemeral_public_key: Option<PublicKey>,

    nonce: H256,
    remote_nonce: Option<H256>,


}
