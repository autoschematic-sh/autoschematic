use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SealingConfig {
    /// Curve used for elliptic key operations.
    /// Currently only "secp256k1" is supported.
    pub curve: String,
    /// AEAD scheme used to encrypt the secret.
    /// Currently only "chacha20poly1305" is supported.
    pub aead: String,
}

impl Default for SealingConfig {
    fn default() -> Self {
        Self {
            curve: String::from("secp256k1"),
            aead: String::from("chacha20poly1305"),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SealedSecret {
    /// Domain of the autoschematic server against which this secret was sealed
    pub server_domain: String,
    /// ID of the public key that was used to seal this secret
    pub server_pubkey_id: String,
    /// Public part of the ephemeral ECDSA keypair used in conjunction with the server key to derive
    /// the shared  used to encrypt the secret
    pub ephemeral_pubkey: String,
    /// Salt used for symmetric key derivation 
    pub salt: String,
    /// Nonce used for symmetric encryption
    pub nonce: String,
    /// Resulting ciphertext encrypted with the selected AEAD
    pub ciphertext: String,
}
