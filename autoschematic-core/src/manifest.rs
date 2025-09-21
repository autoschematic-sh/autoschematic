use serde::{Deserialize, Serialize};

// A Connector Manifest file (connector.ron)
// defines the type of connector (binary-tarpc, binary-grpc, etc etc)
// and specifies the executable path.
// Connector manifests should exist at the root of a repo.

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ConnectorManifest {
    pub name: String,
    pub r#type: String,
    pub executable_name: String,
}
