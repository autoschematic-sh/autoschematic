//! Helper library to store your current working directory when changing to a different directory,
//! then changing back to the original directory once the helper object goes out of scope.

use std::env;
use std::path;

#[derive(Debug)]
pub struct ChangeWorkingDirectory {
    previous_directory: path::PathBuf,
}

impl ChangeWorkingDirectory {
    /// Store the current working directory, then change it to the supplied path. When the struct
    /// goes out of scope, the current working directory will be changed back to what it originally
    /// was.
    ///
    /// ```rust
    /// use std::env;
    /// use chwd::ChangeWorkingDirectory;
    /// use std::io;
    ///
    /// fn main() -> Result<(), std::io::Error>
    /// {
    ///     {
    ///         let _dir_change = ChangeWorkingDirectory::change(&env::temp_dir())?;
    ///         // Do something in the temp dir
    ///     }
    ///
    ///     // _dir_change has gone out of scope, you will be back where you started.
    ///     Ok(())
    /// }
    /// ```
    pub fn change(new_directory: &impl AsRef<path::Path>) -> Result<Self, std::io::Error> {
        let current_working_directory = env::current_dir()?;

        env::set_current_dir(new_directory)?;

        Ok(ChangeWorkingDirectory {
            previous_directory: current_working_directory,
        })
    }
}

impl Drop for ChangeWorkingDirectory {
    fn drop(&mut self) {
        // Panics in drops are bad, especially if the drop is called when code has panicked
        // elsewhere already. We'll stick with printing something to stderr for now...
        if let Err(e) = env::set_current_dir(&self.previous_directory) {
            eprintln!(
                "Unable to change directory back to {:?} due to error {}",
                &self.previous_directory, &e
            );
        }
    }
}
