use handler::Hasher;
use tokio::{fs::File, io::AsyncReadExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file = File::open("./dupa").await?;

    let mut hasher = Hasher::new(handler::HashType::Sha256, file);

    let mut cnt = String::new();
    hasher.read_to_string(&mut cnt).await?;

    println!("cnt: {:?}", cnt);
    println!("hash: {:?}", hex::encode(hasher.finalize()));

    Ok(())
}
