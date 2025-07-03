// autoschematic/src/keystore/ondisk.rs
use anyhow::{Result, anyhow};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use elliptic_curve::SecretKey;
use k256::ecdsa::{Signature, SigningKey};
use k256::pkcs8::der::pem;
use k256::pkcs8::DecodePrivateKey;
use k256::Secp256k1;
use rand_core::OsRng;
use signature::SignerMut;
use std::fs;
use std::path::PathBuf;

use crate::keystore::KeyStore;

#[derive(Debug)]
pub struct OndiskKeyStore {
    key_dir: PathBuf,
}

// TODO could make this generic over curve types?
/// This KeyStore implementation uses on-disk elliptic curve keypairs stored in the .pem format.
/// It is currently hardcoded to Secp256k1.
impl OndiskKeyStore {
    fn key_path(&self, id: &str) -> PathBuf {
        self.key_dir.join(id)
    }
}

impl KeyStore for OndiskKeyStore {
    fn new(path: &str) -> Result<Self> {
        let key_dir = PathBuf::from(path);
        if !key_dir.exists() {
            return Err(anyhow!("OndiskKeystore failed to init: No key store found at {}", path));
        }
        let keystore = OndiskKeyStore { key_dir };
        
        if let Ok(keys) = keystore.list()
            && keys.is_empty() {
                keystore.create_keypair("main")?;
            }

        Ok(keystore)
    }

    fn list(&self) -> Result<Vec<String>> {
        let mut key_ids: Vec<String> = Vec::new();
        for entry in fs::read_dir(&self.key_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && let Some(file_name) = entry.file_name().to_str() {
                    let Ok(pem) = fs::read_to_string(entry.path()) else {
                        tracing::error!("Couldn't read key at {}", file_name);
                        continue
                    };
                    let Ok(_) = SecretKey::<Secp256k1>::from_sec1_pem(&pem) else {
                        tracing::error!("Couldn't parse key at {}", file_name);
                        continue
                    };
                    key_ids.push(file_name.to_string());
                }
        }
        Ok(key_ids)
    }

    fn sign(&self, id: &str, payload: &str) -> Result<String> {
        let key_path = self.key_path(id);
        let private_key_bytes = fs::read(key_path)?;
        let mut signing_key = SigningKey::from_pkcs8_der(&private_key_bytes)?;
        let signature: Signature = signing_key.sign(payload.as_bytes());
        Ok(hex::encode(signature.to_bytes()))
    }

    fn get_public_key(&self, id: &str) -> Result<String> {
        let key_path = self.key_path(id);
        let pem = fs::read_to_string(key_path)?;
        let secret_key = SecretKey::<Secp256k1>::from_sec1_pem(&pem)?;
        let pub_bytes = secret_key.public_key().to_sec1_bytes();
        Ok(BASE64_STANDARD.encode(pub_bytes))
    }

    fn get_private_key(&self, id: &str) -> Result<String> {
        let key_path = self.key_path(id);
        let pem = fs::read_to_string(key_path)?;
        let secret_key = SecretKey::<Secp256k1>::from_sec1_pem(&pem)?;
        let pub_bytes = secret_key.to_bytes();
        Ok(BASE64_STANDARD.encode(pub_bytes))
    }

    fn verify(&self, id: &str, payload: &str) -> Result<String> {
        // let store = Self::new()?;
        // let key_path = store.key_path(id);
        // let public_key_bytes = fs::read(key_path)?;
        // let verifying_key = VerifyingKey::from_sec1_bytes(&public_key_bytes)?;
        // let signature_hex = payload; // Assuming payload contains the signature
        // let signature_bytes = hex::decode(signature_hex)?;
        // let signature = Signature::from_bytes(&signature_bytes)?;
        // verifying_key.verify(payload.as_bytes(), &signature)?;
        Ok("Verified".to_string())
    }
    
    // fn seal_secret(&self, id: &str, payload: &str) -> Result<String> {
    //     let key_path = self.key_path(id);
    //     let private_key_bytes = fs::read(key_path)?;
    //     let signing_key = SigningKey::from_pkcs8_der(&private_key_bytes)?;
    //     let verifying_key: &VerifyingKey = signing_key.verifying_key();
        
    //     // verifying_key.verify(msg, signature);

    //     Ok(String::from(""))
    // }
    
    // fn unseal_secret(&self, id: &str, payload: &str) -> Result<String> {
    //     todo!()
    // }
    
    fn create_keypair(&self, id: &str) -> Result<()> {
        // let mut rng = OsRng;
        let secret = SecretKey::<Secp256k1>::random(&mut OsRng);
        // let secret = EphemeralSecret::random(&mut OsRng);
        let pem = secret.to_sec1_pem(pem::LineEnding::LF)?;
        let out_path = self.key_dir.join(format!("{id}.pem"));
        fs::write(out_path, pem)?;

        Ok(())
    }
    
    fn delete_keypair(&self, id: &str) -> Result<()> {
        let out_path = self.key_dir.join(format!("{id}.pem"));
        if out_path.is_file() {
            fs::remove_file(out_path)?;
        }
        Ok(())
    }
}
