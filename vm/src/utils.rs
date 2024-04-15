use thiserror::Error;
use tokio_serde::{formats::SymmetricalJson, Framed, SymmetricallyFramed};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use futures_util::{SinkExt, TryStreamExt};

#[derive(Error, Debug)]
pub enum UtilitiesError {
    #[error("Stream is closed")]
    StreamIsClosed(),

    #[error("Serde read error")]
    SerdeReadError(#[source] std::io::Error),

    #[error("Serde write error")]
    SerdeWriteError(#[source] std::io::Error)
}

pub async fn serde_read<T: DeserializeOwned + Unpin>(stream: impl AsyncRead + Unpin) -> Result<T, UtilitiesError> {
    let length_delimited = FramedRead::new(stream, LengthDelimitedCodec::new());
    let mut deserialized = SymmetricallyFramed::new(length_delimited, SymmetricalJson::<T>::default());
    let obj = deserialized.try_next().await
        .map_err(UtilitiesError::SerdeReadError)?
        .ok_or(UtilitiesError::StreamIsClosed())?;
    Ok(obj)
}

pub async fn serde_write(stream: impl AsyncWrite + Unpin, obj: impl Serialize + Unpin) -> Result<(), UtilitiesError> {
    let length_delimited = FramedWrite::new(stream, LengthDelimitedCodec::new());
    let mut serialized = SymmetricallyFramed::new(length_delimited, SymmetricalJson::default());
    serialized.send(obj).await.map_err(UtilitiesError::SerdeWriteError)
}
