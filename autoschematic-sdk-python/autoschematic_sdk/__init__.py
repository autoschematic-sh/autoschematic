from __future__ import annotations

import asyncio
import signal
import sys


from autoschematic_sdk.addr import form_addr_object, form_path, match_addr
from .error import InvalidAddr
from .server import create_connector_server
from .types import (
    Connector,
    Diagnostic,
    DiagnosticPosition,
    DiagnosticSpan,
    DocIdent,
    EnumVariantIdent,
    FieldIdent,
    FilterResponse,
    GetDocResponse,
    GetResponse,
    OpExecResponse,
    PlanResponseElement,
    ReadOutput,
    Skeleton,
    StructIdent,
    UnbundleItem,
    VirtToPhyDeferred,
    VirtToPhyNotPresent,
    VirtToPhyNull,
    VirtToPhyPresent,
    VirtToPhyResponse,
)

__all__ = [
    "connector_main",
    "Connector",
    "FilterResponse",
    "GetResponse",
    "PlanResponseElement",
    "OpExecResponse",
    "ReadOutput",
    "VirtToPhyNotPresent",
    "VirtToPhyDeferred",
    "VirtToPhyPresent",
    "VirtToPhyNull",
    "VirtToPhyResponse",
    "Skeleton",
    "DocIdent",
    "StructIdent",
    "FieldIdent",
    "EnumVariantIdent",
    "GetDocResponse",
    "DiagnosticPosition",
    "DiagnosticSpan",
    "Diagnostic",
    "InvalidAddr",
    "UnbundleItem",
    "match_addr",
    "form_addr_object",
    "form_path",
]


async def _run(connector_class: type[Connector]) -> None:
    name = sys.argv[1]
    prefix = sys.argv[2]
    socket = sys.argv[3]
    # error_dump = sys.argv[4]  # reserved for future use

    connector = connector_class(name, prefix)
    server = await create_connector_server(connector, socket)

    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stop.set)

    await stop.wait()
    await server.stop(grace=5)


def connector_main(connector_class: type[Connector]) -> None:
    """Entry point for an autoschematic connector.

    Call this from your connector's ``__main__`` with your ``Connector``
    subclass::

        from autoschematic_sdk import connector_main
        from my_connector import MyConnector

        connector_main(MyConnector)
    """
    asyncio.run(_run(connector_class))