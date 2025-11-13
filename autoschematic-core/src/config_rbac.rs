use std::collections::HashMap;

use crate::macros::FieldTypes;
use autoschematic_macros::FieldTypes;
use documented::{Documented, DocumentedFields, DocumentedVariants};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Documented, DocumentedFields)]
/// The primary RBAC configuration for Autoschematic.
/// This config is used to determine access rights when running an Autoschematic
/// cluster
pub struct AutoschematicRbacConfig {
    /// A map of role names and definitions.
    pub roles: HashMap<String, Role>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Documented, DocumentedFields, FieldTypes)]
/// A role definition that certain users may assume.
pub struct Role {
    /// These users may assume this role and take any action granted to it.
    pub users: Vec<User>,
    /// In the following prefixes, define the level of access to grant this user in the form of
    /// a PrefixGrant.
    pub prefixes: HashMap<String, PrefixGrant>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Documented, DocumentedVariants)]
/// A user definition, identifying a particular user.
pub enum User {
    /// A GitHub user with username `username`.
    GithubUser { username: String },
    // GithubOrganizationUser { organization: String, username: String },
}

#[derive(Clone, Default, Debug, Deserialize, Serialize, PartialEq, PartialOrd, DocumentedVariants)]
/// Defines the actions permissible in the prefix.
pub enum Grant {
    /// Users may not do anything.
    #[default]
    None,
    /// Users may only plan, import, pull-state, etc, and not apply or take any other
    /// action that modifies infrastructure.
    ReadOnly,
    /// Users may take any action, including apply and task-exec, to modify infrastructure,
    /// if and only if their PR is approved by a different user who has `role`.
    ApplyIfApprovedBy { role: String },
    /// Users may take any action, including apply and task-exec,
    /// and may modify infrastructure without the need for approval on their PR.
    Apply,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Documented, DocumentedFields, FieldTypes)]
/// A PrefixGrant defines the level of access that this role can operate with
/// on the prefix
pub struct PrefixGrant {
    /// The level of access to grant in this prefix.
    pub grant: Grant,

    #[serde(default)]
    /// If set, the grant is limited only to these connectors.
    pub connectors: Option<Vec<String>>,
}

impl<'a> AutoschematicRbacConfig {
    pub fn roles_for_user(&'a self, user: &User) -> Vec<&'a Role> {
        let mut res = Vec::new();
        for role in self.roles.values() {
            for role_user in &role.users {
                if role_user == user {
                    res.push(role);
                }
            }
        }

        res
    }

    pub fn grants_for_prefix(&'a self, user: &User, prefix: &str) -> Vec<&'a PrefixGrant> {
        let roles = self.roles_for_user(user);

        let mut prefix_grants = Vec::new();

        for role in roles {
            if let Some(prefix_grant) = role.prefixes.get(prefix) {
                prefix_grants.push(prefix_grant);
            }
        }

        prefix_grants
    }

    /// Tests if the user has permission to read/plan/pull-state etc with this prefix and connector.
    pub fn allows_read(&self, user: &User, prefix: &str, connector: &str) -> bool {
        for grant in self.grants_for_prefix(user, prefix) {
            if grant.grant >= Grant::ReadOnly
                && let Some(ref connectors) = grant.connectors
                && connectors.contains(&connector.into())
            {
                return true;
            }
        }

        false
    }

    /// Tests if the user has permission to apply with this prefix and connector without the need for
    /// another role's approval.
    pub fn allows_apply_without_approval(&self, user: &User, prefix: &str, connector: &str) -> bool {
        for grant in self.grants_for_prefix(user, prefix) {
            if grant.grant == Grant::Apply
                && let Some(ref connectors) = grant.connectors
                && connectors.contains(&connector.into())
            {
                return true;
            }
        }

        false
    }

    /// Tests if the user _could_ apply with this prefix and connector, but would require approval from another
    /// role to do so. Does not determine if that other role has given approval or not.
    pub fn allows_apply_with_approval(&self, user: &User, prefix: &str, connector: &str) -> bool {
        for grant in self.grants_for_prefix(user, prefix) {
            if let Grant::ApplyIfApprovedBy { .. } = &grant.grant
                && let Some(ref connectors) = grant.connectors
                && connectors.contains(&connector.into())
            {
                return true;
            }
        }

        false
    }

    /// Tests if the user is permitted to apply with this prefix and connector, given that the set of users in `approving_users` have
    /// approved the PR/changeset. Tests for each of those user's
    pub fn allows_apply_if_approved_by(&self, user: &User, prefix: &str, connector: &str, approving_users: &Vec<User>) -> bool {
        for grant in self.grants_for_prefix(user, prefix) {
            // We have a grant that allows apply, but only if approved by a user with `role`.
            if let Grant::ApplyIfApprovedBy { role } = &grant.grant {
                // For every user who approved this action...
                for approving_user in approving_users {
                    // ...go through all of that approving user's roles...
                    for approving_user_role in self.roles_for_user(approving_user) {
                        // ...and if they have that role, and we match connector constraints (if any), we're good to go!
                        if self.roles.get(role) == Some(approving_user_role) {
                            if let Some(ref connectors) = grant.connectors {
                                if connectors.contains(&connector.into()) {
                                    return true;
                                }
                            } else {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn user(name: &str) -> User {
        User::GithubUser {
            username: name.to_string(),
        }
    }

    fn role_with_prefix(users: Vec<User>, prefix: &str, grant: Grant, connectors: Option<Vec<&str>>) -> Role {
        let mut prefixes = HashMap::new();
        prefixes.insert(
            prefix.to_string(),
            PrefixGrant {
                grant,
                connectors: connectors.map(|v| v.into_iter().map(String::from).collect()),
            },
        );
        Role { users, prefixes }
    }

    #[test]
    fn roles_for_user_empty() {
        let cfg = AutoschematicRbacConfig::default();
        assert!(cfg.roles_for_user(&user("finn")).is_empty());
    }

    #[test]
    fn roles_for_user_single_and_multiple() {
        let mut cfg = AutoschematicRbacConfig::default();
        let r1 = role_with_prefix(vec![user("finn")], "pre", Grant::None, None);
        let r2 = role_with_prefix(vec![user("jake")], "pre", Grant::None, None);
        cfg.roles.insert("role1".into(), r1.clone());
        cfg.roles.insert("role2".into(), r2.clone());

        let roles_finn = cfg.roles_for_user(&user("finn"));
        assert_eq!(roles_finn.len(), 1);
        assert_eq!(roles_finn[0], &r1);

        let roles_jake = cfg.roles_for_user(&user("jake"));
        assert_eq!(roles_jake.len(), 1);
        assert_eq!(roles_jake[0], &r2);

        let roles_unknown = cfg.roles_for_user(&user("carol"));
        assert!(roles_unknown.is_empty());
    }

    #[test]
    fn grants_for_prefix_filters_correctly() {
        let mut cfg = AutoschematicRbacConfig::default();
        let r1 = role_with_prefix(vec![user("finn")], "pre1", Grant::ReadOnly, Some(vec!["c1"]));
        let r2 = role_with_prefix(vec![user("finn")], "pre2", Grant::Apply, Some(vec!["c2"]));
        cfg.roles.insert("r1".into(), r1.clone());
        cfg.roles.insert("r2".into(), r2.clone());

        let gs_pre1 = cfg.grants_for_prefix(&user("finn"), "pre1");
        assert_eq!(gs_pre1.len(), 1);
        assert_eq!(gs_pre1[0], &r1.prefixes["pre1"]);

        let gs_pre2 = cfg.grants_for_prefix(&user("finn"), "pre2");
        assert_eq!(gs_pre2.len(), 1);
        assert_eq!(gs_pre2[0], &r2.prefixes["pre2"]);

        let gs_none = cfg.grants_for_prefix(&user("finn"), "other");
        assert!(gs_none.is_empty());
    }

    #[test]
    fn allows_read_positive_and_negative() {
        let u = user("finn");
        let mut cfg = AutoschematicRbacConfig::default();
        let r = role_with_prefix(vec![u.clone()], "p", Grant::ReadOnly, Some(vec!["c1", "c2"]));
        cfg.roles.insert("r".into(), r);

        assert!(cfg.allows_read(&u, "p", "c1"));
        assert!(cfg.allows_read(&u, "p", "c2"));
        assert!(!cfg.allows_read(&u, "p", "c3"));
        assert!(!cfg.allows_read(&u, "x", "c1"));
    }

    #[test]
    fn allows_read_requires_connectors_some() {
        let u = user("finn");
        let mut cfg = AutoschematicRbacConfig::default();
        let mut role = Role::default();
        role.users.push(u.clone());
        role.prefixes.insert(
            "p".into(),
            PrefixGrant {
                grant: Grant::ReadOnly,
                connectors: None,
            },
        );
        cfg.roles.insert("r".into(), role);

        assert!(!cfg.allows_read(&u, "p", "any"));
    }

    #[test]
    fn allows_apply_without_approval_positive_and_negative() {
        let u = user("jake");
        let mut cfg = AutoschematicRbacConfig::default();
        let r = role_with_prefix(vec![u.clone()], "pre", Grant::Apply, Some(vec!["conn"]));
        cfg.roles.insert("r".into(), r);

        assert!(cfg.allows_apply_without_approval(&u, "pre", "conn"));
        assert!(!cfg.allows_apply_without_approval(&u, "pre", "other"));
        assert!(!cfg.allows_apply_without_approval(&u, "x", "conn"));
    }

    #[test]
    fn allows_apply_without_approval_needs_connectors() {
        let u = user("jake");
        let mut cfg = AutoschematicRbacConfig::default();
        let mut role = Role::default();
        role.users.push(u.clone());
        role.prefixes.insert(
            "pre".into(),
            PrefixGrant {
                grant: Grant::Apply,
                connectors: None,
            },
        );
        cfg.roles.insert("r".into(), role);
        assert!(!cfg.allows_apply_without_approval(&u, "pre", "conn"));
    }

    #[test]
    fn allows_apply_with_approval_flag_behavior() {
        let u = user("eve");
        let mut cfg = AutoschematicRbacConfig::default();
        let r = role_with_prefix(
            vec![u.clone()],
            "pp",
            Grant::ApplyIfApprovedBy { role: "admin".into() },
            Some(vec!["c"]),
        );
        cfg.roles.insert("pp-role".into(), r);

        assert!(cfg.allows_apply_with_approval(&u, "pp", "c"));
        assert!(!cfg.allows_apply_with_approval(&u, "pp", "x"));
        assert!(!cfg.allows_apply_with_approval(&u, "other", "c"));
    }

    #[test]
    fn allows_apply_with_approval_needs_connectors() {
        let u = user("eve");
        let mut cfg = AutoschematicRbacConfig::default();
        let mut role = Role::default();
        role.users.push(u.clone());
        role.prefixes.insert(
            "pp".into(),
            PrefixGrant {
                grant: Grant::ApplyIfApprovedBy { role: "admin".into() },
                connectors: None,
            },
        );
        cfg.roles.insert("r".into(), role);

        assert!(!cfg.allows_apply_with_approval(&u, "pp", "any"));
    }

    #[test]
    fn allows_apply_if_approved_by_success() {
        let finn = user("finn");
        let jake = user("jake");
        let mut cfg = AutoschematicRbacConfig::default();

        // finn may apply if approved by role "approver"
        let mut role_user = Role::default();
        role_user.users.push(finn.clone());
        role_user.prefixes.insert(
            "pp".into(),
            PrefixGrant {
                grant: Grant::ApplyIfApprovedBy { role: "approver".into() },
                connectors: Some(vec!["c1".into()]),
            },
        );
        // jake is an approver
        let mut role_approver = Role::default();
        role_approver.users.push(jake.clone());

        cfg.roles.insert("user".into(), role_user);
        cfg.roles.insert("approver".into(), role_approver);

        assert!(cfg.allows_apply_if_approved_by(&finn, "pp", "c1", &vec![jake.clone()]));
    }

    #[test]
    fn allows_apply_if_approved_by_connector_mismatch_or_no_approval() {
        let finn = user("finn");
        let jake = user("jake");
        let mut cfg = AutoschematicRbacConfig::default();

        let mut role_user = Role::default();
        role_user.users.push(finn.clone());
        role_user.prefixes.insert(
            "pp".into(),
            PrefixGrant {
                grant: Grant::ApplyIfApprovedBy { role: "approver".into() },
                connectors: Some(vec!["c1".into()]),
            },
        );
        let mut role_approver = Role::default();
        role_approver.users.push(jake.clone());

        cfg.roles.insert("user".into(), role_user);
        cfg.roles.insert("approver".into(), role_approver);

        // wrong connector
        assert!(!cfg.allows_apply_if_approved_by(&finn, "pp", "bad", &vec![jake.clone()]));
        // wrong approver
        assert!(!cfg.allows_apply_if_approved_by(&finn, "pp", "c1", &vec![finn.clone()]));
        // no approvers
        assert!(!cfg.allows_apply_if_approved_by(&finn, "pp", "c1", &vec![]));
    }

    #[test]
    fn allows_apply_if_approved_by_no_connectors_allows_any() {
        let finn = user("finn");
        let jake = user("jake");
        let mut cfg = AutoschematicRbacConfig::default();

        let mut role_user = Role::default();
        role_user.users.push(finn.clone());
        role_user.prefixes.insert(
            "pp".into(),
            PrefixGrant {
                grant: Grant::ApplyIfApprovedBy { role: "approver".into() },
                connectors: None,
            },
        );
        let mut role_approver = Role::default();
        role_approver.users.push(jake.clone());

        cfg.roles.insert("user".into(), role_user);
        cfg.roles.insert("approver".into(), role_approver);

        // connectors == None allows any connector
        assert!(cfg.allows_apply_if_approved_by(&finn, "pp", "foo", &vec![jake.clone()]));
        assert!(cfg.allows_apply_if_approved_by(&finn, "pp", "bar", &vec![jake.clone()]));
    }

    #[test]
    fn grants_for_prefix_multiple_roles_combined() {
        let u = user("mul");
        let mut cfg = AutoschematicRbacConfig::default();
        let r1 = role_with_prefix(vec![u.clone()], "x", Grant::ReadOnly, Some(vec!["c"]));
        let r2 = role_with_prefix(vec![u.clone()], "x", Grant::Apply, Some(vec!["c"]));
        cfg.roles.insert("r1".into(), r1.clone());
        cfg.roles.insert("r2".into(), r2.clone());

        let gs = cfg.grants_for_prefix(&u, "x");
        assert_eq!(gs.len(), 2);
        assert!(gs.contains(&&r1.prefixes["x"]));
        assert!(gs.contains(&&r2.prefixes["x"]));
    }
}
