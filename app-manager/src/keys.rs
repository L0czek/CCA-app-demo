use thiserror::Error;

// TODO: This is a mock up, implement key sealing later

pub type Key = [u8; 32];

#[derive(Error, Debug)]
pub enum KeyManagerError {

}

pub struct KeyManager {

}

impl KeyManager {
    pub fn new() -> Result<Self, KeyManagerError> {
        Ok(Self {})
    }

    pub fn realm_sealing_key(&self) -> Result<Key, KeyManagerError> {
        Ok([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31])
    }

    pub fn application_sealing_key(&self) -> Result<Key, KeyManagerError> {
        Ok([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31])
    }
}
