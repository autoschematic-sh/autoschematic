

use serde::{Deserialize, Serialize};


#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AutoschematicRbacConfig {
    pub roles: Vec<Role>
}


#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Role {
    pub name: String,
    pub users: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct GrantPrefix {
    pub prefix_name: String,
    pub roles: Vec<String>
}