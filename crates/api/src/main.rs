#[tokio::main]
async fn main() -> anyhow::Result<()> {
    plumb::identity!("SANTI").map_err(anyhow::Error::msg)?;
    api::run().await
}
