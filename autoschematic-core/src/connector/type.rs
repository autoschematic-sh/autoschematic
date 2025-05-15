use std::path::PathBuf;

type ShortName = String;
type ClassName = String;

#[derive(Debug, Clone)]
pub enum ConnectorType {
    Python(PathBuf, ClassName),
    BinaryTarpc(PathBuf, ShortName),
    LockFile(PathBuf, ShortName),
    // Lock(Rc<ConnectorType>)
}
