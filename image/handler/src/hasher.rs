use pin_project::pin_project;
use sha2::{digest::DynDigest, Digest, Sha256, Sha512};
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use crate::common::HashType;

#[pin_project]
pub struct Hasher<T: AsyncRead> {
    hash: Box<dyn DynDigest>,

    #[pin]
    inner: T,
}

impl<T: AsyncRead> Hasher<T> {
    pub fn new(ty: HashType, inner: T) -> Self {
        let hash = match ty {
            HashType::Sha256 => Box::new(Sha256::new()) as Box<dyn DynDigest>,
            HashType::Sha512 => Box::new(Sha512::new()) as Box<dyn DynDigest>
        };

        Self {
            hash,
            inner,
        }
    }

    pub fn finalize(&mut self) -> Box<[u8]> {
        self.hash.finalize_reset()
    }
}

impl<T: AsyncRead> AsyncRead for Hasher<T> {
    fn poll_read(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.project();

        let previous = buf.filled().len();
        match this.inner.poll_read(cx, buf) {
            std::task::Poll::Pending => std::task::Poll::Pending,

            std::task::Poll::Ready(v) => {
                let data = &buf.filled()[previous..];
                this.hash.update(data);
                std::task::Poll::Ready(v)
            }
        }
    }
}
