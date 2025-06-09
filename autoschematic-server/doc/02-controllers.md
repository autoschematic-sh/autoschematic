In order to develop a controller, the implementer must define 4 key data structures.

### The Controller itself (impl Controller)

Contains domain-specific init, config and bookkeeping data.
For instance, the SnowflakeController::new() picks up its account-specific data
and other config 

### Resource Address (impl ResourceAddress)

Defines how 

### Resource (impl Resource)
Defines the representation of a resource managed by this controller,
as well as its mappings to/from serialized representations like yaml or ron.

The combination of a Controller and a Resource Address should uniquely and globally identify any 
resource that the controller is capable of managing.

### Op 
Enum representations of possible actions. A really simple controller might only have 1 op.
Ops should be serializable. `Controller::plan(addr, current, desired)` produces a series of ops 
corresponding to the series of actions that the controller will take to put the resource at `addr` into the `desired` state.