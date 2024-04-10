use std::fmt::Display;

#[derive(Debug, Copy, Clone)]
pub enum HashType {
    Sha256,
    Sha512
}

impl Display for HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashType::Sha256 => write!(f, "sha256"),
            HashType::Sha512 => write!(f, "sha512")
        }
    }
}
