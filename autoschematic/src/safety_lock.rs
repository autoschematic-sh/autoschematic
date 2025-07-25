use anyhow::bail;
use autoschematic_core::util::repo_root;

pub fn check_safety_lock() -> anyhow::Result<()> {
    let repo_root = repo_root()?;
    let safety_lock_path = repo_root.join(".autoschematic.safety.lock");
    if safety_lock_path.is_file() {
        bail!("The safety lock is set, preventing any operation that would modify infrastructure.");
    } else {
        return Ok(());
    }
}

pub fn set_safety_lock() -> anyhow::Result<()> {
    let repo_root = repo_root()?;
    let safety_lock_path = repo_root.join(".autoschematic.safety.lock");
    if safety_lock_path.is_file() {
        eprintln!("Safety lock already set.");
        return Ok(());
    } else {
        eprintln!(
            "Safety lock set. It is now not possible to modify infrastructure with `autoschematic apply` or task execution."
        );
        std::fs::write(safety_lock_path, "LOCK")?;
        return Ok(());
    }
}

pub fn unset_safety_lock() -> anyhow::Result<()> {
    let repo_root = repo_root()?;
    let safety_lock_path = repo_root.join(".autoschematic.safety.lock");
    if safety_lock_path.is_file() {
        std::fs::remove_file(safety_lock_path)?;
        eprintln!(
            "Safety lock unset. It is now possible to modify infrastructure with `autoschematic apply` or task execution."
        );
        return Ok(());
    } else {
        eprintln!("Safety lock not set.");
        return Ok(());
    }
}
