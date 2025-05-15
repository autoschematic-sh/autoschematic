from autoschematic_connector.lib import *
try:
    # Try and load our hooks from rust land
    from autoschematic_connector_hooks import read_secret
except:
    pass