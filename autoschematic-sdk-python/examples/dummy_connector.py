"""
A minimal example connector that returns static data.
"""

import json

from autoschematic_sdk import (
    Connector,
    FilterResponse,
    GetResponse,
    PlanResponseElement,
    OpExecResponse,
    connector_main,
    match_addr,
    InvalidAddr,
)

from dataclasses import dataclass

WIDGET_ADDR = match_addr("widgets/[name].ron")


@dataclass
class Widget:
    color: str
    weight: int


class DummyConnector(Connector):
    def __init__(self, name: str, prefix: str) -> None:
        """
        __init__() generally shouldn't fail, even if config is invalid. That's for the second-stage init() method.
        (Note: in rust, this is called new(), hence the slightly confusing naming with init()...)
        """

        self.name = name
        self.prefix = prefix
        self.widgets: dict[str, bytes] = {
            "widgets/foo.ron": b'Widget(color: "red", weight: 10)',
            "widgets/bar.ron": b'Widget(color: "blue", weight: 25)',
        }

    async def init(self) -> None:
        """
        init() is where more involved setup, like validating config files or initializing clients, should go.
        init() can fail and report an error efficiently to the host, whereas __init__() failing will silently cut off
        the communication at the socket without explanation.
        """
        print(f"DummyConnector '{self.name}' initialized for prefix '{self.prefix}'")

    async def filter(self, addr: str) -> FilterResponse:
        """
        filter() is where your connector declares whether it "owns" a given file. `addr`, as in all other
         methods, is passed in without its prefix.
         For example, in the prefix autoschematic-sdk-python/examples:
         filter(widgets/foo.ron) -> FilterResponse.RESOURCE
         filter(widgets/bar.ron) -> FilterResponse.RESOURCE
         filter(widgets/bar.txt) -> FilterResponse.NONE
         filter(something/else.ron) -> FilterResponse.NONE

        There are other types of FilterResponses, but NONE and RESOURCE are the most common.
        If you return FilterResponse.CONFIG for a given file, the LSP will restart your connector
         whenever that file is modified, allowing you to reload config in init(). 
         
        `filter()` will be cached by the host, and should essentially be static (though it is allowed to depend on any 
         file marked as FilterResponse.CONFIG, so that the host can reload the connector when
         any such file changes).

        """
        if WIDGET_ADDR.match(addr):
            return FilterResponse.RESOURCE
        return FilterResponse.NONE

    async def list(self, subpath: str) -> list[str]:
        """
        In list(), your connector might query a remote API and return a list of paths (addresses) corresponding
         to resources that exist. Here, our dummy connector just returns the static list of widgets that it stores internally.
        list() is rarely called except during import, and enumerating every possible resource addressable by a connector is likely to take time.
        `subpath` is used to constrain the search for performance. You can safely ignore the subpath variable. See other connectors for how to use `subpath` effectively.
        See the Connector trait in connector.rs for more information about `subpath`.
        """
        return [addr for addr in self.widgets]

    async def get(self, addr: str) -> GetResponse:
        """
        get() should fetch the current state of a resource at `addr` (if present) and return it 
        in its serialized representation. get() also returns a dict of output values if any are present.
        """

        if addr in self.widgets:
            widget_addr = WIDGET_ADDR.match(addr)
            assert widget_addr

            return GetResponse(
                exists=True,
                resource_definition=self.widgets[addr],
                outputs={"widget_id": f"wid-{widget_addr.group("name")}"},
            )
        return GetResponse(exists=False)

    async def plan(
        self,
        addr: str,
        current: bytes | None,
        desired: bytes | None,
    ) -> list[PlanResponseElement]:
        """
        plan() is where your connector computes what operations need to be carried out to go from current state to a desired state.
        Here, `current` and `desired` are the serialized (result from get() and on-disk, respectively) states.
        plan() will return a list of PlanResponseElements, each of which might be executed by 'autoschematic apply' in sequence.
        Each PlanResponseElement corresponds to a connector op. Just like resource bodies, the serialized format of connector ops is also
        a design choice left up to the connector. The one below just uses static json.
        """
        if WIDGET_ADDR.match(addr) is None:
            raise InvalidAddr(addr)

        match [current, desired]:
            case [None, None]:
                pass
            # desired == None implies deleting the resource at `addr`
            case [current, None]:
                return [
                    PlanResponseElement(
                        op_definition='{{"action": "delete"}}',
                        writes_outputs=[],
                        friendly_message="Delete widget",
                    )
                ]
            # current == None implies the resource at `addr` doesn't yet exist
            case [None, desired]:
                return [
                    PlanResponseElement(
                        op_definition='{{"action": "create"}}',
                        writes_outputs=["widget_id"],
                        friendly_message="Create widget",
                    )
                ]
            case [current, desired] if current != desired:
                return [
                    PlanResponseElement(
                        op_definition='{{"action": "update"}}',
                        writes_outputs=["widget_id"],
                        friendly_message="Update widget",
                    )
                ]

        return []

    async def op_exec(self, addr: str, op: str) -> OpExecResponse:
        """
        op_exec() is where your connector will execute connector ops as returned by plan().
        In op_exec(), you'll actually make the e.g. AWS calls to create, modify, delete etc 
        the resources as specified in the op definition. 
        You'll receive `op` in serialized form, so you'll need to parse it yourself.
        """
        widget_addr = WIDGET_ADDR.match(addr)
        
        if widget_addr:
            dec_op = json.loads(op)
            
            action = dec_op.get('action')

            return OpExecResponse(
                outputs={"widget_id": f"wid-{widget_addr.group("name")}"},
                friendly_message=f"Executed {action} on {addr}",
            )
        else:
            raise InvalidAddr(addr)


if __name__ == "__main__":
    connector_main(DummyConnector)
