use clap::{command, ArgAction, Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(disable_help_subcommand = true)]
#[command(name = "autoschematic", about = "A tool for managing remote resources")]
pub struct AutoschematicCommand {
    #[command(subcommand)]
    pub command: AutoschematicSubcommand,
}

pub const HELP: &str = "
`autoschematic plan [-p prefix] [-c connector] [-s subpath]`
Determine how to set the current state of resources to their desired state as described in this pull request.
Use [-p|--prefix] to limit scope to a single prefix.
Use [-c|--connector] to limit scope to a single connector.
Use [-s|--subpath] to limit scope to a subfolder of the repository.

`autoschematic apply [-p prefix] [-c connector] [-s subpath] [--merge]`
Carry out the actions described by the most recent 'autoschematic plan'.
The options [-p|--prefix], [-c|--connector] and [-s subpath] have the same effect as for 'autoschematic plan'.  
The options [-p|--prefix], [-c|--connector] and [-s subpath] must match the most recent invocation of 'autoschematic plan'
If --merge is given, the pull request will be merged on success.

`autoschematic import [-p prefix] [-c connector] [-s subpath] [--overwrite bool=false]`
For each connector, carry out a search of all of its remote resources and import them into the repository.
Autoschematic will commit the new files found and push the commit with a message pointing to the comment that invoked it.
Note that this command may take a long time to run, particularly if you have, for instance, large AWS accounts with many resources present.
Filtering using [-s|--subpath] may reduce this time, but it is not guaranteed.
If --overwrite is given, imported resources will be written and commited to the pull request even 
if they already exist in the repository.
The options [-p|--prefix], [-c|--connector] and [-s subpath] have the same effect as for 'autoschematic plan'.  

`autoschematic pull-state [-p prefix] [-c connector] [-s subpath]` 
For each resource in this pull request, fetch its current state and import it 
into the repository.
The options [-p|--prefix], [-c|--connector] and [-s subpath] have the same effect as for 'autoschematic plan'.  

`autoschematic import-skeletons [-p prefix] [-c connector] [-s subpath] [--overwrite bool=false]`
For each connector, fetch all of its \"skeleton\" resources and import them into the repo.
Skeleton resources are example template resources you can use in order to understand
the format and path structure of resources that can be created by installed connectors.
The options [-p|--prefix], [-c|--connector] and [-s subpath] have the same effect as for 'autoschematic plan'.  

`autoschematic help`
Print this message.

`autoschematic safety [off]`
Turn the safety on or off.
With the safety on, autoschematic will not run any operations for this pull request until the safety is turned off.
";

#[derive(Subcommand, Debug)]
pub enum AutoschematicSubcommand {
    /// Import remote resources that aren't yet defined. Optionally filter by {path}.
    /// Use --overwrite to overwrite existing resource definitions if present.
    Import {
        /// Optional: run for a single prefix by name
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,
        
        /// Overwrite existing resource definitions if they are already present
        #[arg(long, action = ArgAction::SetTrue)]
        overwrite: bool,
    },

    /// Display the series of operations needed to apply the changeset.
    Plan {

        /// Optional path (can be a glob) to filter the changeset.
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,
    },

    /// Execute the necessary operations to apply the changeset.
    Apply {
        /// Optional path (can be a glob) to specify which subset of the changeset to apply.
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,
    },

    /// For each resource in the pull request, get its current state and import it into the repo.
    PullState {
        /// Optional path (can be a glob) to specify which subset of the changeset to apply.
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,

        /// If objects in the changeset do not exist remotely, delete them.
        #[arg(long, action = ArgAction::SetTrue)]
        delete: bool,
    },

    Help {
    }

}
