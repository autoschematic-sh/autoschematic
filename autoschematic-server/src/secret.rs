// use std::{
//     collections::HashMap,
//     fs::{create_dir_all, File},
//     io::Write,
//     path::{Path, PathBuf},
// };

// use itertools::Itertools;
// use rand::{distr::Alphanumeric, Rng};
// use secrecy::{ExposeSecret, SecretBox};
// use serde::{Deserialize, Serialize};

// #[derive(Debug, Serialize, Deserialize)]
// pub struct SealedSecret {
//     pub server_domain: String,
//     pub server_pubkey_id: String,
//     pub ephemeral_pubkey: String,
//     pub salt: String,
//     pub nonce: String,
//     pub ciphertext: String,
// }

/* // TODO: Could the sealed secrets store be a bind mount?
// Or do we need to expose a special hashmap that handles sealing new secrets
//  as they're created by the connector?


pub struct SealingConfig {
    active_seals: Vec<u8>,
}

pub fn seal_dir_path() -> anyhow::Result<PathBuf> {
    match std::env::var("AUTOSCHEMATIC_SECRET_SEAL_DIR") {
        Ok(path) => Ok(PathBuf::from(path)),
        Err(_) => {
            let current_exe = std::env::current_exe()?;
            Ok(current_exe
                .parent()
                .unwrap()
                .join("secret_seals")
                .to_path_buf())
        }
    }
}

pub fn create_seal(index: u8) -> Result<(), anyhow::Error> {
    let seal_dir_path = seal_dir_path()?;

    let seal_path = seal_dir_path.join(format!("seal{}.secret", index));

    let mut f = File::create_new(&seal_path)?;
    let s: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(512)
        .map(char::from)
        .collect();

    f.write_all(&s.as_bytes())?;

    Ok(())
}

pub fn load_seal(index: u8) -> Result<SecretBox<String>, anyhow::Error> {
    let seal_dir_path = seal_dir_path()?;

    let seal_path = seal_dir_path.join(format!("seal{}.secret", index));

    let secret_box = SecretBox::new(Box::new(std::fs::read_to_string(seal_path)?));

    Ok(secret_box)
}

pub fn delete_seal(index: u8) -> Result<(), anyhow::Error> {
    let seal_dir_path = seal_dir_path()?;

    let seal_path = seal_dir_path.join(format!("seal{}.secret", index));

    std::fs::remove_file(seal_path)?;

    Ok(())
}

impl SealingConfig {
    pub fn push_seal(&mut self) -> anyhow::Result<u8> {
        let max = *self.active_seals.iter().max().unwrap_or(&0);

        create_seal(max + 1)?;
        self.active_seals.push(max + 1);

        Ok(max + 1)
    }

    pub fn pop_seal(&mut self) -> anyhow::Result<Option<u8>> {
        let Some(min_index) = self.active_seals.iter().position_min() else {
            // Nothin' to pop!
            return Ok(None);
        };

        let min = self.active_seals[min_index];
        delete_seal(min)?;
        self.active_seals.remove(min_index);

        Ok(Some(min))
    }

    pub fn seal_secret(
        &self,
        dir: &Path,
        secret_path: &Path,
        secret: SecretBox<String>,
    ) -> Result<Vec<PathBuf>, anyhow::Error> {
        let mut res = Vec::new();
        // For each active seal, encrypt
        for seal_index in &self.active_seals {
            let sealed_secret_dir = &dir.join(format!("seal{}", seal_index));
            create_dir_all(sealed_secret_dir)?;
            let sealed_secret_path = &sealed_secret_dir.join(secret_path);

            let seal = load_seal(*seal_index)?;

            let mut map = HashMap::new();
            map.insert(String::from("body"), secret.expose_secret().to_string());

            let mut f = File::create_new(sealed_secret_path)?;
            f.write_all(sealed_secrets::encode(&map, seal.expose_secret())?.as_bytes())?;

            res.push(sealed_secret_path.to_path_buf());
        }

        Ok(res)
    }

    pub fn unseal_secret(
        &self,
        dir: &Path,
        secret_path: &Path,
    ) -> Result<SecretBox<String>, anyhow::Error> {
        let mut res = Vec::new();
        for seal_index in &self.active_seals {
            let sealed_secret_dir = &dir.join(format!("seal{}", seal_index));
            let sealed_secret_path = &sealed_secret_dir.join(secret_path);

            if std::fs::exists(sealed_secret_path)? {
                let seal = load_seal(*seal_index)?;
                let body = std::fs::read_to_string(sealed_secret_path)?;
                let sec = sealed_secrets::decode(&body, seal.expose_secret())?;
                // TOOO I'm sure this violates the whole point of the secretbox,
                // which is to zero out the data on .drop() ....
                let Some(secret) = sec.get("body") else {
                    return Err(anyhow::Error::msg(format!("Malformed secret: no 'body' at {:?}", secret_path)))
                };

                return Ok(SecretBox::new(Box::new(secret.to_string())))
            }
            res.push(sealed_secret_path.to_path_buf());
        }

        Err(anyhow::Error::msg(format!("No seal found to decode secret at {:?}", secret_path)))
    }

    //For each secret in the secret store,
    // for each active seal, if the secret has not yet been encrypted
    // with that seal, find an active seal that can decrypt it, and
    // encrypt a new copy with the "new" seal.
    // Inverse: a function that deletes sealed secrets signed by keys that are no longer active
    // So, then, to cycle secrets you'd do:
    // push, refresh, pop, recycle
    pub fn refresh_sealed_secrets() {
    }

    pub fn remove_defunct_sealed_secrets() {

    }
}
 */
