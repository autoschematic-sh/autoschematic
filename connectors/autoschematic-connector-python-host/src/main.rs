use autoschematic_core::tarpc_bridge::tarpc_connector_main;
use connector::PythonConnector;

pub mod connector;
pub mod util;


#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    tarpc_connector_main::<PythonConnector>().await?;
    Ok(())
}
