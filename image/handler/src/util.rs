use std::path::Path;

use serde::Deserialize;
use tokio::{fs::File, io::{AsyncRead, AsyncReadExt}};

use crate::{HashType, Hasher, ImageError, Result};


pub async fn read_measured(ty: HashType, path: &Path) -> Result<(String, Box<[u8]>)> {
    let mut file = File::open(path).await.map_err(ImageError::FileOpenError)?;
    let mut reader = Hasher::new(ty, &mut file);
    let mut content = String::new();
    reader.read_to_string(&mut content).await.map_err(ImageError::FileReadError)?;
    Ok((content, reader.finalize()))
}

pub async fn discard_rest<T: AsyncRead + Unpin>(mut io: T) {
    let mut buf = Vec::with_capacity(1024);
    buf.resize(1024, 0u8);

    while let Ok(v) = io.read(&mut buf).await {
        if v == 0 {
            break;
        }
    }
}

