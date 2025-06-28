use std::path::PathBuf;

use anyhow::bail;
use http_body_util::BodyExt;
use itertools::Itertools;
use octocrab::{models::repos::DiffEntryStatus, params::repos::Commitish, Octocrab};
use serde::Deserialize;

use crate::RON;

#[derive(Clone)]
pub struct Object {
    pub owner: String,
    pub repo: String,
    pub head_sha: String,
    pub filename: PathBuf,
    pub diff_status: DiffEntryStatus,
}

impl Object {
    pub async fn parse_ron<T>(&self, client: &Octocrab) -> Result<T, anyhow::Error>
    where
        T: for<'a> Deserialize<'a>,
    {
        let Some(filename) = self.filename.to_str() else {
            bail!("parse_ron: bad filename!")
        };

        let mut body = client
            .repos(self.owner.clone(), self.repo.clone())
            .raw_file(Commitish(self.head_sha.clone()), filename)
            .await?;

        let mut config_str = String::new();

        while let Some(next) = body.frame().await {
            let frame = next?;
            if let Some(chunk) = frame.data_ref() {
                config_str.push_str(std::str::from_utf8(chunk)?);
            }
        }

        let config = RON.from_str(&config_str)?;

        Ok(config)
    }
}

/// Ordering rules:
/// Create/Modify always comes before Delete.
/// Create/Modify are sorted shortest path first
/// Delete are sorted longest path first
pub fn sort_objects_by_apply_order(objects: &Vec<Object>) -> Vec<Object> {
    let created_obj = objects
        .iter()
        .filter(|o| o.diff_status != DiffEntryStatus::Removed);

    let deleted_obj = objects
        .iter()
        .filter(|o| o.diff_status == DiffEntryStatus::Removed);

    let mut created_obj: Vec<&Object> = created_obj.sorted_by_key(|o| &o.filename).collect();
    created_obj.reverse();
    let mut deleted_obj: Vec<&Object> = deleted_obj.sorted_by_key(|o| &o.filename).collect();

    let mut objects = created_obj;
    objects.append(&mut deleted_obj);

    objects.into_iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use rand::seq::SliceRandom;

    use super::*;
    

    #[test]
    fn test_sort_objects_by_apply_order() {
        // Create a vector with 32+ objects with various diff statuses and filenames
        let mut objects = Vec::new();
        
        // Helper function to create test objects
        let create_object = |filename: &str, status: DiffEntryStatus| -> Object {
            Object {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                head_sha: "test-sha".to_string(),
                filename: PathBuf::from(filename),
                diff_status: status,
            }
        };
        
        // Create objects with various paths and statuses
        // 16 objects with Added status
        objects.push(create_object("a/file1.rs", DiffEntryStatus::Added));
        objects.push(create_object("a/file2.rs", DiffEntryStatus::Added));
        objects.push(create_object("a/b/file3.rs", DiffEntryStatus::Added));
        objects.push(create_object("a/b/file4.rs", DiffEntryStatus::Added));
        objects.push(create_object("a/b/c/file5.rs", DiffEntryStatus::Added));
        objects.push(create_object("a/b/c/file6.rs", DiffEntryStatus::Added));
        objects.push(create_object("x/file7.rs", DiffEntryStatus::Added));
        objects.push(create_object("x/file8.rs", DiffEntryStatus::Added));
        objects.push(create_object("x/y/file9.rs", DiffEntryStatus::Added));
        objects.push(create_object("x/y/file10.rs", DiffEntryStatus::Added));
        objects.push(create_object("x/y/z/file11.rs", DiffEntryStatus::Added));
        objects.push(create_object("x/y/z/file12.rs", DiffEntryStatus::Added));
        objects.push(create_object("config/file13.ron", DiffEntryStatus::Added));
        objects.push(create_object("config/file14.ron", DiffEntryStatus::Added));
        objects.push(create_object("config/templates/file15.ron", DiffEntryStatus::Added));
        objects.push(create_object("config/templates/file16.ron", DiffEntryStatus::Added));
        
        // 8 objects with Modified status
        objects.push(create_object("m/file1.rs", DiffEntryStatus::Modified));
        objects.push(create_object("m/file2.rs", DiffEntryStatus::Modified));
        objects.push(create_object("m/n/file3.rs", DiffEntryStatus::Modified));
        objects.push(create_object("m/n/file4.rs", DiffEntryStatus::Modified));
        objects.push(create_object("p/file5.rs", DiffEntryStatus::Modified));
        objects.push(create_object("p/file6.rs", DiffEntryStatus::Modified));
        objects.push(create_object("p/q/file7.rs", DiffEntryStatus::Modified));
        objects.push(create_object("p/q/file8.rs", DiffEntryStatus::Modified));
        
        // 8 objects with Removed status
        objects.push(create_object("r/file1.rs", DiffEntryStatus::Removed));
        objects.push(create_object("r/file2.rs", DiffEntryStatus::Removed));
        objects.push(create_object("r/s/file3.rs", DiffEntryStatus::Removed));
        objects.push(create_object("r/s/file4.rs", DiffEntryStatus::Removed));
        objects.push(create_object("t/file5.rs", DiffEntryStatus::Removed));
        objects.push(create_object("t/file6.rs", DiffEntryStatus::Removed));
        objects.push(create_object("t/u/file7.rs", DiffEntryStatus::Removed));
        objects.push(create_object("t/u/file8.rs", DiffEntryStatus::Removed));
        
        objects.shuffle(&mut rand::rng());
        
        // Sort the objects
        let sorted_objects = sort_objects_by_apply_order(&objects);
        
        // Verify the sorting rules:
        // 1. All Created/Modified objects should come before all Removed objects
        let first_removed_idx = sorted_objects
            .iter()
            .position(|o| o.diff_status == DiffEntryStatus::Removed);
        
        if let Some(idx) = first_removed_idx {
            // All objects before this index should be Added or Modified
            for i in 0..idx {
                assert_ne!(sorted_objects[i].diff_status, DiffEntryStatus::Removed);
            }
            
            // All objects from this index onwards should be Removed
            for i in idx..sorted_objects.len() {
                assert_eq!(sorted_objects[i].diff_status, DiffEntryStatus::Removed);
            }
        }
        
        // 2. Check that Created/Modified objects are sorted by shortest path first
        // Find the range of Created/Modified objects
        let non_removed_count = sorted_objects
            .iter()
            .filter(|o| o.diff_status != DiffEntryStatus::Removed)
            .count();
        
        // Check that paths get progressively longer or equal within the Created/Modified section
        for i in 1..non_removed_count {
            let prev_components = sorted_objects[i-1].filename.components().count();
            let curr_components = sorted_objects[i].filename.components().count();
            
            // The previous object's path should be the same length or shorter than the current one
            // (because we reversed the created_obj list)
            assert!(prev_components <= curr_components, 
                "Created/Modified objects not sorted correctly: {:?} should come after {:?}",
                sorted_objects[i-1].filename, sorted_objects[i].filename);
        }
        
        // 3. Check that Removed objects are sorted by longest path first
        // If there are any removed objects
        if let Some(idx) = first_removed_idx {
            // Check that paths get progressively shorter or equal within the Removed section
            for i in (idx + 1)..sorted_objects.len() {
                let prev_components = sorted_objects[i-1].filename.components().count();
                let curr_components = sorted_objects[i].filename.components().count();
                
                // The previous object's path should be the same length or longer than the current one
                assert!(prev_components >= curr_components,
                    "Removed objects not sorted correctly: {:?} should come before {:?}",
                    sorted_objects[i-1].filename, sorted_objects[i].filename);
            }
        }
        
        // Ensure we have at least 32 objects as required
        assert!(objects.len() >= 32, "Test requires at least 32 objects");
        assert_eq!(sorted_objects.len(), objects.len(), "Sort should preserve object count");
    }
}
