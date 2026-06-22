use anyhow::{Context, Result};
use rand::thread_rng;
use rsa::{
    pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding},
    RsaPrivateKey, RsaPublicKey,
};
use std::path::PathBuf;

fn get_key_paths() -> Result<(PathBuf, PathBuf)> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let base_dir = home.join(".kumo");
    Ok((
        base_dir.join("private_key.pem"),
        base_dir.join("public_key.pem"),
    ))
}

pub fn get_or_create_keypair() -> Result<(String, String)> {
    let (priv_path, pub_path) = get_key_paths()?;

    if priv_path.exists() && pub_path.exists() {
        let priv_pem = std::fs::read_to_string(&priv_path)?;
        let pub_pem = std::fs::read_to_string(&pub_path)?;
        return Ok((priv_pem, pub_pem));
    }

    let mut rng = thread_rng();
    let private_key =
        RsaPrivateKey::new(&mut rng, 2048).context("Failed to generate RSA private key")?;
    let public_key = RsaPublicKey::from(&private_key);

    let private_pem = private_key
        .to_pkcs8_pem(LineEnding::LF)
        .context("Failed to encode private key to PEM")?
        .to_string();
    let public_pem = public_key
        .to_public_key_pem(LineEnding::LF)
        .context("Failed to encode public key to PEM")?;

    if let Some(parent) = priv_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&priv_path, &private_pem)?;
    std::fs::write(&pub_path, &public_pem)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&priv_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&priv_path, perms)?;
    }

    Ok((private_pem, public_pem))
}

pub fn sign_payload(private_key_pem: &str, payload: &str) -> Result<String> {
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::Pkcs1v15Sign;
    use sha2::{Digest, Sha256};

    let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)
        .context("Failed to parse private key from PEM")?;

    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    let hashed = hasher.finalize();

    let signature = private_key
        .sign(Pkcs1v15Sign::new::<Sha256>(), &hashed)
        .context("Signing failed")?;

    let hex_signature = signature
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    Ok(hex_signature)
}
