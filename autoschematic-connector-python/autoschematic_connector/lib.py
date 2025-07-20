from dataclasses import dataclass
from typing import Dict, List, Optional
from abc import ABC, abstractmethod


@dataclass
class GetResourceResponse:
    resource_definition: str
    outputs: Optional[Dict[str, str]] = None


@dataclass
class PlanResponseElement:
    op_definition: str
    friendly_message: Optional[str] = None


@dataclass
class OpExecResponse:
    outputs: Optional[Dict[str, str]] = None
    friendly_message: Optional[str] = None


# This function will be overridden when run under the Autoschematic Connector runtime,
#  allowing connectors fine-grained access to decrypt sealed secrets within the repository.
def read_secret(path: str) -> str | None:
    return None


class Connector(ABC):
    # Note: Connectors are instantiated with the working directory at the root of the 
    #  repository, not at their specific prefix.
    # However, all paths (or addrs) passed to get(...), op_exec(...) etc are stripped
    #  of their prefix to .
    @abstractmethod
    def __init__(self, prefix):
        pass


    # If a path (addr) decodes to a valid addr for an object that could be managed
    #  by this connector, return True; else return False.
    # E.G. 
    #  `snowflake/databases/SOME_DB/database.sql` -> True
    #  `something/else/unrelated` -> False
    @abstractmethod
    def filter(self, addr: str) -> bool:
        return False

    # Under a given subpath within the prefix under which this connector is instantiated,
    #  return recursively all of the addresses that correspond to objects that
    #  exist remotely.
    # This connector implementation does not actually do any subpath filtering:
    #  this is valid behaviour, and Autoschematic will additionally filter
    #  the returned addresses after it calls list(). 
    # The subpath argument can be used by connectors as an optional optimization for lookup.
    # For example, if this connector implemented subpath filtering, 
    #  list("snowflake/databases/TEST_DB/PUBLIC/tables) would return
    #  all the table definitions in the TEST_DB.PUBLIC schema.
    @abstractmethod
    def list(self, subpath: str) -> List[str]:
        return []

    # At a given path (or addr) within the prefix under which this connector is 
    #  instantiated, get the representation of the object that corresponds to 
    #  that addr, or None if no such object exists.
    @abstractmethod
    def get(self, addr: str) -> GetResourceResponse | None:
        return None


    # At a given path(addr), with current and desired state, 
    #  return a list of ConnectorOps which will enact the changes
    #  from current -> desired when executed.
    # Connector implementations can decide on any representation for their 
    #  ConnectorOps, which will be passed to op_exec(...) to execute them.
    @abstractmethod
    def plan(self, addr: str, current: str | None, desired: str | None) -> List[PlanResponseElement]:
        return None
        

    # At a given path(addr),
    #  execute a connector op as returned by plan(...).
    # op_exec(...) may return a hashmap of strings that represent 
    #  the outputs from the op's execution. For instance, when creating a Route53 Hosted Zone,
    #  op_exec(...) can store {"hosted_zone_id": "..."}.
    # Autoschematic will store the hashmap as a json file next to the original resource file.
    # For example, "aws/route53/hosted_zones/example.com/zone.yaml"
    #  would produce "aws/route53/hosted_zones/example.com/.zone.yaml.out.json",
    #  which would contain the hosted zone ID it just created.
    # Some fields, if present, are treated specially by Autoschematic. 
    # These fields are:
    #  `friendly_message`: A human-readable message for operators.
    #   For example, "Created Hosted Zone example.com"
    #    or "Destroyed EC2 Instance with ID ..."
    @abstractmethod
    def op_exec(self, addr_s: str, op_s: str) -> OpExecResponse | None:
        return None
