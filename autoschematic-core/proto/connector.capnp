@0xa9d3e0f9278b0fc1;

# Represents a single key/value pair in a map.
# In Rust, the value is Option<String>; here a null value means None.
struct OutputEntry {
  key @0 :Text;
  value @1 :Text;  # may be null
}

# Mirrors GetResourceOutput:
# - `resourceDefinition` is required.
# - `outputs` is an optional list (null means None).
struct GetResourceOutput {
  resourceDefinition @0 :Text;
  outputs @1 :List(OutputEntry);  # If outputs is null, that means None.
}

# Mirrors OpPlanOutput.
struct OpPlanOutput {
  opDefinition @0 :Text;
  friendlyMessage @1 :Text;  # Optional: null indicates no message.
}

# Mirrors OpExecOutput.
struct OpExecOutput {
  outputs @0 :List(OutputEntry);  # Optional list; null means None.
  friendlyMessage @1 :Text;         # Optional friendly message.
}

# The Connector interface exposes the core methods.
# Paths are represented as Text (e.g. the string form of the path).
interface Connector {
  # filter(addr): returns true if the connector cares about this address.
  filter @0 (addr :Text) -> (result :Bool);
  
  # list(subpath): returns a list of file paths as Text.
  list @1 (subpath :Text) -> (paths :List(Text));
  
  # get(addr): returns the current state of the resource at `addr` (or null if not present).
  get @2 (addr :Text) -> (output :GetResourceOutput);
  
  # plan(addr, current, desired):
  # - current and desired are optional strings (null means None).
  # - returns a list of planned operations.
  plan @3 (addr :Text, current :Text, desired :Text) -> (plans :List(OpPlanOutput));
  
  # opExec(addr, op): executes an operation, returning its result.
  opExec @4 (addr :Text, op :Text) -> (output :OpExecOutput);
}

# Optionally, you could define a factory to instantiate connectors.
# The `outbox` channel isn’t modeled here since it would require a separate host interface.
# interface ConnectorFactory {
#  newConnector @0 (name :Text, prefix :Text) -> (connector :Connector);
#}
