use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use git2::{
    Cred, FetchOptions, IndexAddOption, PushOptions, RemoteCallbacks, Repository, Status, StatusOptions,
    build::{CheckoutBuilder, RepoBuilder},
};
use secrecy::{ExposeSecret, SecretBox};

pub async fn clone_repo(
    owner: &str,
    repo: &str,
    path: &Path,
    head_ref: &str,
    token: &SecretBox<str>,
) -> Result<Repository, anyhow::Error> {
    let owner_path = path.join(owner);
    fs::create_dir_all(&owner_path)?;

    let repo_path = path.join(owner).join(repo);

    let repo_url = format!("https://github.com/{owner}/{repo}.git");

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
        // Typically, GitHub expects:
        //   - Username: "x-access-token"
        //   - Password: "<YOUR_TOKEN>"
        Cred::userpass_plaintext("x-access-token", token.expose_secret())
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    fetch_opts.depth(1);
    let _checkout_opts = CheckoutBuilder::new();

    let repository = match Repository::open(&repo_path) {
        Ok(repository) => repository,
        Err(_) => RepoBuilder::new()
            .fetch_options(fetch_opts)
            .branch(head_ref)
            .clone(&repo_url, &repo_path)
            .context("RepoBuilder::new()")?,
    };

    // let submodules = repository.submodules()?;

    // let mut callbacks = RemoteCallbacks::new();
    // callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
    //     // Typically, GitHub expects:
    //     //   - Username: "x-access-token"
    //     //   - Password: "<YOUR_TOKEN>"
    //     Cred::userpass_plaintext("x-access-token", token.expose_secret())
    // });

    // let mut fetch_opts = FetchOptions::new();
    // fetch_opts.remote_callbacks(callbacks);
    // fetch_opts.depth(1);
    // let _checkout_opts = CheckoutBuilder::new();

    // let mut submodule_update_options = SubmoduleUpdateOptions::new();
    // let update_opts = submodule_update_options.fetch(fetch_opts);

    // for mut submodule in submodules {
    //     tracing::info!(
    //         "Cloning submodule {:?} at {:?}",
    //         submodule.name(),
    //         submodule.path()
    //     );
    //     submodule.init(false)?;
    //     submodule.update(true, Some(update_opts))?;
    // }

    repository.remote_set_url("origin", &repo_url)?;

    // repository.reset(
    //     repository.find_("HEAD")?.,
    //     git2::ResetType::Hard,
    //     None,
    // ).context("git reset")?;

    Ok(repository)
}

pub async fn checkout_new_branch(repo_path: &Path, branch_name: &str) -> Result<Repository, anyhow::Error> {
    let Ok(repository) = Repository::open(repo_path) else {
        bail!("No repository at {}", &repo_path.to_str().unwrap_or_default())
    };

    {
        let branch = repository.branch(branch_name, &repository.head()?.peel_to_commit()?, false)?;

        // repository.checkout_tree(&obj, None)?;
        repository.checkout_tree(&branch.get().peel(git2::ObjectType::Tree)?, None)?;
        repository.set_head(&format!("refs/heads/{branch_name}"))?;
    }

    Ok(repository)
}

pub async fn checkout_branch(repo_path: &Path, branch_name: &str) -> anyhow::Result<()> {
    let Ok(repository) = Repository::open(repo_path) else {
        bail!("No repository at {}", &repo_path.to_str().unwrap_or_default())
    };

    let (object, reference) = repository.revparse_ext(branch_name)?;

    repository.checkout_tree(&object, None)?;

    match reference {
        // gref is an actual reference like branches or tags
        Some(gref) => {
            repository.set_head(gref.name().unwrap())?;
        }
        // this is a commit, not a reference
        None => {
            repository.set_head_detached(object.id())?;
        }
    }

    Ok(())
}

pub fn git_add(repo_path: &Path, path: &Path) -> anyhow::Result<()> {
    // tracing::error!("git_add({:?}, {:?})", repo_path, path);

    // let repo_path = path.join(owner).join(repo);
    //

    let Ok(repository) = Repository::open(repo_path) else {
        bail!("No repository at {}", &repo_path.to_str().unwrap_or_default())
    };

    let mut index = repository.index()?;
    index.add_all([path], IndexAddOption::default(), None)?;
    index.write()?;
    Ok(())
}

pub fn git_commit(repo_path: &Path, username: &str, email: &str, message: &str) -> anyhow::Result<()> {
    let Ok(repository) = Repository::open(repo_path) else {
        bail!("No repository at {}", &repo_path.to_str().unwrap_or_default())
    };

    let mut index = repository.index()?;
    let oid = index.write_tree()?;
    let parent_commit = repository.head()?.peel_to_commit()?;
    let tree = repository.find_tree(oid)?;
    let sig = git2::Signature::now(username, email)?;
    repository.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])?;

    Ok(())
}

pub fn git_commit_and_push(repo_path: &Path, head_ref: &str, token: &SecretBox<str>, message: &str) -> anyhow::Result<()> {
    let Ok(repository) = Repository::open(repo_path) else {
        bail!("No repository at {}", &repo_path.to_str().unwrap_or_default())
    };

    let mut index = repository.index()?;
    let oid = index.write_tree()?;
    let parent_commit = repository.head()?.peel_to_commit()?;
    let tree = repository.find_tree(oid)?;
    let sig = git2::Signature::now("autoschematic", "apply@autoschematic.sh")?;
    repository.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])?;

    let mut remote = repository.find_remote("origin")?;

    let refspec = format!("refs/heads/{head_ref}:refs/heads/{head_ref}");

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
        // Typically, GitHub expects:
        //   - Username: "x-access-token"
        //   - Password: "<YOUR_TOKEN>"
        Cred::userpass_plaintext("x-access-token", token.expose_secret())
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    remote.push::<&str>(&[&refspec], Some(&mut push_options))?;
    Ok(())
}

pub fn pull_with_rebase(repo_path: &Path, branch_name: &str, token: &SecretBox<str>) -> Result<(), anyhow::Error> {
    let Ok(repository) = Repository::open(repo_path) else {
        bail!("No repository at {}", repo_path.to_str().unwrap_or_default())
    };

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
        // Typically, GitHub expects:
        //   - Username: "x-access-token"
        //   - Password: "<YOUR_TOKEN>"
        Cred::userpass_plaintext("x-access-token", token.expose_secret())
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    // fetch_opts.depth(1);

    let mut remote = repository.find_remote("origin")?;

    remote.fetch(&[branch_name], Some(&mut fetch_opts), None)?;

    let fetch_head = repository.find_reference("FETCH_HEAD")?;

    let fetch_ref = repository.reference_to_annotated_commit(&fetch_head)?;

    let branch_ref_name = format!("refs/heads/{branch_name}");
    let mut branch_ref = repository.find_reference(&branch_ref_name)?;

    let msg = format!("Fast-Forward: Setting {} to id: {}", branch_ref_name, fetch_ref.id());

    branch_ref.set_target(fetch_ref.id(), &msg)?;

    repository.set_head(&branch_ref_name)?;

    repository.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;

    Ok(())
}

pub fn get_head_sha(repo_path: &Path) -> anyhow::Result<String> {
    if let Ok(repository) = Repository::open(repo_path) {
        let head = repository.head()?;
        Ok(head.peel_to_commit()?.id().to_string())
    } else {
        bail!("No repository at {}", repo_path.to_str().unwrap_or_default())
    }
}

pub fn get_staged_files() -> Result<Vec<PathBuf>, git2::Error> {
    // Discover the repository by looking in `.` and upwards
    let repo = Repository::discover(".")?;

    // Configure status options to only look at changes staged in the index
    let mut status_opts = StatusOptions::new();
    status_opts.show(git2::StatusShow::Index);
    status_opts.include_untracked(false).renames_head_to_index(true);

    // Gather statuses
    let statuses = repo.statuses(Some(&mut status_opts))?;

    // Filter for any kind of index change
    let mut staged = Vec::new();
    for entry in statuses.iter() {
        let s = entry.status();

        let is_staged = s.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        );

        if is_staged && let Some(path) = entry.path() {
            staged.push(PathBuf::from(path));
        }

        if s.intersects(Status::INDEX_RENAMED)
            && let Some(new_path) = entry.head_to_index().and_then(|d| d.new_file().path())
        {
            staged.push(PathBuf::from(new_path));
        }
    }
    Ok(staged)
}
