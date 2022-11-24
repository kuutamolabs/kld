use aes_gcm_siv::aead::{Aead, AeadCore, OsRng};
use aes_gcm_siv::{Aes256GcmSiv, Key, KeyInit, Nonce};
use settings::Settings;

pub struct Cipher {
    aes: Aes256GcmSiv,
}

impl Cipher {
    pub fn new(settings: &Settings) -> Cipher {
        let key = Key::<Aes256GcmSiv>::from_slice(&settings.database_encryption_key.as_ref());
        let cipher = Aes256GcmSiv::new(&key);

        Cipher { aes: cipher }
    }

    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let nonce = Aes256GcmSiv::generate_nonce(&mut OsRng);
        let ciphertext = self.aes.encrypt(&nonce, data).expect("encryption failure!");
        let mut result = nonce.to_vec();
        result.extend(&ciphertext);
        result
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Vec<u8> {
        let (nonce, ciphertext) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce);
        self.aes
            .decrypt(nonce, ciphertext.as_ref())
            .expect("decryption failure!")
    }
}

#[cfg(test)]
mod test {
    use test_utils::test_settings;

    use crate::cipher::Cipher;

    #[test]
    pub fn test_cipher() {
        let settings = test_settings();

        let cipher = Cipher::new(&settings);
        let message = b"plaintext message plaintext message plaintext message";
        let ciphertext = cipher.encrypt(message);

        let cipher = Cipher::new(&settings);
        let plaintext = cipher.decrypt(&ciphertext);
        assert_eq!(&plaintext, message);
    }
}
