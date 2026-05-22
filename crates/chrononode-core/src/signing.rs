use ed25519_dalek::Signer;
use ed25519_dalek::{Signature, SignatureError, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use std::path::Path;

pub struct OperatorKeypair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl OperatorKeypair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    pub fn verify(&self, signature: &Signature, message: &[u8]) -> Result<(), SignatureError> {
        self.verifying_key.verify_strict(message, signature)
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    pub fn signing_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn from_env() -> Option<Self> {
        let Ok(val) = std::env::var("CHRONONODE_OPERATOR_KEY") else {
            return None;
        };
        let bytes = hex::decode(val.trim()).ok()?;
        let seed: [u8; 32] = bytes.try_into().ok()?;
        Some(Self::from_seed(&seed))
    }

    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let seed: [u8; 32] = bytes.try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "key file must be 32 bytes")
        })?;
        Ok(Self::from_seed(&seed))
    }

    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = self.signing_key_bytes();
        std::fs::write(path, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }
        Ok(())
    }
}

pub fn verify_signature(
    pubkey_bytes: &[u8; 32],
    signature_bytes: &[u8; 64],
    message: &[u8],
) -> bool {
    let Ok(verifying_key) = VerifyingKey::from_bytes(pubkey_bytes) else {
        return false;
    };
    let signature = Signature::from_bytes(signature_bytes);
    verifying_key.verify_strict(message, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate_sign_verify() {
        let keypair = OperatorKeypair::generate();
        let message = b"test checkpoint root";
        let sig = keypair.sign(message);
        assert!(keypair.verify(&sig, message).is_ok());
    }

    #[test]
    fn test_keypair_rejects_wrong_message() {
        let keypair = OperatorKeypair::generate();
        let sig = keypair.sign(b"original");
        assert!(keypair.verify(&sig, b"tampered").is_err());
    }

    #[test]
    fn test_keypair_roundtrip_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("chrononode_test_key");
        let original = OperatorKeypair::generate();
        original.save_to_file(&path).unwrap();
        let loaded = OperatorKeypair::from_file(&path).unwrap();
        assert_eq!(original.verifying_key_bytes(), loaded.verifying_key_bytes());
        let message = b"roundtrip test";
        let sig = loaded.sign(message);
        assert!(loaded.verify(&sig, message).is_ok());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_verify_signature_standalone() {
        let keypair = OperatorKeypair::generate();
        let message = b"standalone verify";
        let sig = keypair.sign(message);
        let pubkey = keypair.verifying_key_bytes();
        let sig_bytes: [u8; 64] = sig.to_bytes();
        assert!(verify_signature(&pubkey, &sig_bytes, message));
    }
}
