use std::path::Path;

use autoschematic_core::secret::SealedSecret;
use chacha20poly1305::{aead::Aead, AeadCore, KeyInit};
use ecdsa::EncodedPoint;
use elliptic_curve::{ecdh::EphemeralSecret, PublicKey};
use k256::Secp256k1;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;

use crate::config::load_autoschematic_config;



pub async fn seal(domain: &str, prefix: Option<&str>, path: &Path, in_path: Option<&Path>, key_id: Option<&str>) -> anyhow::Result<()> {
    
    let autoschematic_config = load_autoschematic_config()?;
    
    // let prefix = prefix.unwrap_or("autoschematic");
    
    // if !autoschematic_config.prefixes.contains_key(prefix) {
    //     bail!("Prefix {} not found in autoschematic.ron", prefix)
    // }

    let key_id = if key_id.is_some() {
        key_id.unwrap().to_string()
    } else {
        let pubkey_list_json: serde_json::Value = reqwest::Client::new()
            .get(format!("https://{domain}/api/pubkeys"))
            .send()
            .await?
            .json()
            .await?;

        let pubkey_list = pubkey_list_json.as_array().unwrap();

        pubkey_list[0].as_str().unwrap().to_string()
    };

    let pubkey_string_base64 = reqwest::Client::new()
        .get(format!("https://{domain}/api/pubkey/{key_id}"))
        .send()
        .await?
        .text()
        .await?;

    let pubkey_string = base64::decode(pubkey_string_base64)?;

    let server_pubkey: PublicKey<Secp256k1> = PublicKey::from_sec1_bytes(&pubkey_string)?;

    let ephemeral_secret = EphemeralSecret::<Secp256k1>::random(&mut OsRng);
    let ephemeral_pubkey = EncodedPoint::<Secp256k1>::from(ephemeral_secret.public_key());

    let shared_secret = ephemeral_secret.diffie_hellman(&server_pubkey);

    let mut salt = vec![0u8; 32];

    // TODO is 64 bytes a large enough key??
    let mut okm = vec![0u8; 32];
    OsRng.fill_bytes(&mut salt);
    let hkdf_obj = shared_secret.extract::<Sha256>(Some(&salt));
    hkdf_obj.expand(&[], &mut okm).unwrap();

    // let key = chacha20poly1305::ChaCha20Poly1305::generate_key(&mut OsRng);
    let cipher = chacha20poly1305::ChaCha20Poly1305::new_from_slice(&okm).unwrap();
    let nonce = chacha20poly1305::ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let secret_to_seal = if let Some(in_path) = in_path {
        std::fs::read(in_path)?
    } else {
        inquire::Password::new("Secret contents:")
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt()?
        .into_bytes()
    };

    let ciphertext = cipher.encrypt(&nonce, &*secret_to_seal).unwrap();

    let seal = SealedSecret {
        server_domain: domain.to_string(),
        server_pubkey_id: key_id.to_string(),
        ephemeral_pubkey: base64::encode(ephemeral_pubkey.as_bytes()),
        salt: base64::encode(salt),
        nonce: base64::encode(nonce),
        ciphertext: base64::encode(ciphertext),
    };
    
    // // form output path for sealed secret
    // let mut out_path = PathBuf::from(prefix).join(".secret").join(path);
    // if let Some(ext) = path.extension() {
    //     out_path.set_extension(format!("{}.sealed", ext.to_str().unwrap()));
    // } else {
    //     out_path.set_extension("sealed");
    // }
    std::fs::create_dir_all(path.parent().unwrap())?;

    std::fs::write(path, serde_json::to_string_pretty(&[seal])?)?;
    Ok(())
}