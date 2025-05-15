### Account scope
Account scope represents the information associated with a deployment of a controller.
For instance, account scope for a controller that manages AWS resources may include information
such as the AWS account ID, IAM role to assume, etc.
Information associated with account scope should be stored in the Controller implementation's eponymous struct.


### Resource Address
A resource address is an object that identifies a single file within a repo.
A single resource address maps bidirectionally with a single path within a git repository.
The impl defines that mapping. These functions should be pretty simple.

Controller::parse_resource_address(path: PathBuf) -> Self::Address
Controller::dump_resource_address(address: Self::Address) -> PathBuf

### In Combination
The combination of account scope information and the local resource 
address should be enough to globally and unambiguously identify 
any resource managed by a controller.