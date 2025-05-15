use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Result};
use chacha20poly1305::{aead::Aead, AeadCore, KeyInit};
use ecdsa::EncodedPoint;
use elliptic_curve::{
    ecdh::{diffie_hellman, EphemeralSecret}, PublicKey, SecretKey,
};
use k256::Secp256k1;
use ondisk::OndiskKeyStore;
use rand_core::{OsRng, RngCore};
use regex::Regex;
use sha2::Sha256;

use crate::error::{AutoschematicError, AutoschematicErrorType};
use crate::secret::SealedSecret;

pub mod ondisk;

pub trait KeyStore: Send + Sync + std::fmt::Debug {
    fn new(path: &str) -> Result<Self>
    where
        Self: Sized;
    ///List the key IDs present in the KeyStore.
    ///Inactive or deprecated keys are not returned.
    fn list(&self) -> Result<Vec<String>>;
    ///Sign the payload with the key `id`, and
    /// return the result.
    /// Fails if `id` is invalid or if signing fails.
    fn sign(&self, id: &str, payload: &str) -> Result<String>;
    ///Return the public part of the key `id`.
    /// Fails if `id` is invalid.
    fn get_public_key(&self, id: &str) -> Result<String>;
    fn get_private_key(&self, id: &str) -> Result<String>;
    /// Verifies a signed payload against key `id`.
    /// Fails if `id` is invalid or if verification fails.
    fn verify(&self, id: &str, payload: &str) -> Result<String>;

    fn create_keypair(&self, id: &str) -> Result<()>;
    fn delete_keypair(&self, id: &str) -> Result<()>;

    fn seal_secret(&self, domain: &str, id: &str, payload: &str) -> Result<SealedSecret> {
        let pubkey_string_base64 = self.get_public_key(id)?;
        let pubkey_string = base64::decode(pubkey_string_base64)?;
        let server_pubkey: PublicKey<Secp256k1> = PublicKey::from_sec1_bytes(&pubkey_string)?;

        let ephemeral_secret = EphemeralSecret::<Secp256k1>::random(&mut OsRng);
        let ephemeral_pubkey = EncodedPoint::<Secp256k1>::from(ephemeral_secret.public_key());

        let shared_secret = ephemeral_secret.diffie_hellman(&server_pubkey);

        let mut salt = vec![0u8; 32];

        let mut okm = vec![0u8; 32];
        OsRng.fill_bytes(&mut salt);
        let hkdf_obj = shared_secret.extract::<Sha256>(Some(&salt));
        hkdf_obj.expand(&[], &mut okm).unwrap();

        let cipher = chacha20poly1305::ChaCha20Poly1305::new_from_slice(&okm)?;
        let nonce = chacha20poly1305::ChaCha20Poly1305::generate_nonce(&mut OsRng);

        let Ok(ciphertext) = cipher.encrypt(&nonce, payload.as_bytes()) else {
            bail!("Keystore: seal_secret: failed to encrypt")
        };

        let seal = SealedSecret {
            server_domain: domain.to_string(),
            server_pubkey_id: id.to_string(),
            ephemeral_pubkey: base64::encode(ephemeral_pubkey.as_bytes()),
            salt: base64::encode(salt),
            nonce: base64::encode(nonce),
            ciphertext: base64::encode(ciphertext),
        };
        Ok(seal)
    }

    fn unseal_secret(&self, secret: &SealedSecret) -> Result<String> {
        let privkey_string_base64 = self.get_private_key(&secret.server_pubkey_id)?;
        let privkey_string = base64::decode(privkey_string_base64)?;
        let privkey = SecretKey::<Secp256k1>::from_bytes(privkey_string.as_slice().into())?;
        let ephemeral_pubkey = PublicKey::<Secp256k1>::from_sec1_bytes(
            base64::decode(&secret.ephemeral_pubkey)?.as_slice(),
        )?;

        let shared_secret =
            diffie_hellman::<Secp256k1>(privkey.to_nonzero_scalar(), ephemeral_pubkey.as_affine());

        let salt = base64::decode(&secret.salt)?;

        let mut okm = vec![0u8; 32];
        let hkdf_obj = shared_secret.extract::<Sha256>(Some(&salt));
        hkdf_obj.expand(&[], &mut okm).unwrap();

        let cipher = chacha20poly1305::ChaCha20Poly1305::new_from_slice(&okm)?;
        let nonce = base64::decode(&secret.nonce)?;

        let plaintext = cipher
            .decrypt(
                nonce.as_slice().into(),
                base64::decode(&secret.ciphertext)?.as_slice(),
            )
            .unwrap();

        Ok(String::from_utf8(plaintext)?)
    }
    // For each entry in the hashmap,
    // If it matches $secret://some_path/in_therepo
    fn unseal_env_map(&self, env: &HashMap<String, String>) -> anyhow::Result<HashMap<String, String>> {
        let re = Regex::new(r"^secret://(?<path>.+)$")?;

        let mut out_map = HashMap::new();

        for (key, value) in env {
            if let Some(caps) = re.captures(value) {
                let path = PathBuf::from(&caps["path"]);
                let seals: Vec<SealedSecret> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
                //TODO more than one seal support?
                let secret = seals.get(0).unwrap();
                let plaintext = self.unseal_secret(&secret)?;
                
                out_map.insert(key.clone(), plaintext);
            } else {
                out_map.insert(key.clone(), value.clone());
            }
        }

        Ok(out_map)
    }
}

/// Initialize a keystore at a given URI.
/// E.G. ondisk:///some_secure_directory
pub fn keystore_init(name: &str) -> Result<Box<dyn KeyStore>> {
    let re = Regex::new(r"^(?<type>[^:/]+)://(?<path>.+)$")?;

    let Some(caps) = re.captures(name) else {
        return Err(AutoschematicError {
            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
        }
        .into());
    };

    match &caps["type"] {
        "ondisk" => Ok(Box::new(OndiskKeyStore::new(&caps["path"])?)),
        _ => Err(AutoschematicError {
            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
        }
        .into()),
    }
}
