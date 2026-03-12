from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import IntFlag


class FilterResponse(IntFlag):
    NONE = 0
    CONFIG = 1
    RESOURCE = 2
    BUNDLE = 4
    TASK = 8
    METRIC = 16


@dataclass
class GetResponse:
    exists: bool
    resource_definition: bytes | None = None
    virt_addr: str | None = None
    outputs: dict[str, str] | None = None


@dataclass
class PlanResponseElement:
    op_definition: str
    writes_outputs: list[str] = field(default_factory=list)
    friendly_message: str = ""


@dataclass
class OpExecResponse:
    outputs: dict[str, str] | None = None
    friendly_message: str = ""


@dataclass
class ReadOutput:
    addr: str
    key: str


@dataclass
class VirtToPhyNotPresent:
    pass


@dataclass
class VirtToPhyDeferred:
    reads: list[ReadOutput]


@dataclass
class VirtToPhyPresent:
    path: str


@dataclass
class VirtToPhyNull:
    path: str


VirtToPhyResponse = VirtToPhyNotPresent | VirtToPhyDeferred | VirtToPhyPresent | VirtToPhyNull


@dataclass
class Skeleton:
    addr: str
    body: bytes


@dataclass
class DocIdent:
    pass


@dataclass
class StructIdent(DocIdent):
    name: str = ""


@dataclass
class FieldIdent(DocIdent):
    parent: str = ""
    name: str = ""


@dataclass
class EnumVariantIdent(DocIdent):
    parent: str = ""
    name: str = ""


@dataclass
class GetDocResponse:
    has_doc: bool
    type: str = ""
    markdown: str = ""
    fields: list[str] = field(default_factory=list)


@dataclass
class DiagnosticPosition:
    line: int
    col: int


@dataclass
class DiagnosticSpan:
    start: DiagnosticPosition
    end: DiagnosticPosition


@dataclass
class Diagnostic:
    severity: int
    span: DiagnosticSpan
    message: str


@dataclass
class UnbundleItem:
    addr: str
    contents: bytes


class Connector(ABC):
    @abstractmethod
    def __init__(self, name: str, prefix: str) -> None: ...

    @abstractmethod
    async def init(self) -> None: ...

    @abstractmethod
    async def filter(self, addr: str) -> FilterResponse: ...

    @abstractmethod
    async def list(self, subpath: str) -> list[str]: ...

    async def subpaths(self) -> list[str]:
        return ["./"]

    @abstractmethod
    async def get(self, addr: str) -> GetResponse: ...

    @abstractmethod
    async def plan(
        self,
        addr: str,
        current: bytes | None,
        desired: bytes | None,
    ) -> list[PlanResponseElement]: ...

    @abstractmethod
    async def op_exec(self, addr: str, op: str) -> OpExecResponse: ...

    async def addr_virt_to_phy(self, addr: str) -> VirtToPhyResponse:
        return VirtToPhyNull(addr)

    async def addr_phy_to_virt(self, addr: str) -> str | None:
        return addr

    async def get_skeletons(self) -> list[Skeleton]:
        return []

    async def get_docstring(self, addr: str, ident: DocIdent) -> GetDocResponse | None:
        return None

    async def eq(self, addr: str, a: bytes, b: bytes) -> bool:
        return a == b

    async def diag(self, addr: str, a: bytes) -> list[Diagnostic]:
        return []

    async def unbundle(self, addr: str, bundle: bytes) -> list[UnbundleItem]:
        return []