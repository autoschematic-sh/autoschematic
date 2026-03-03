from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class FilterResponseType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    NONE: _ClassVar[FilterResponseType]
    CONFIG: _ClassVar[FilterResponseType]
    RESOURCE: _ClassVar[FilterResponseType]
    BUNDLE: _ClassVar[FilterResponseType]
    TASK: _ClassVar[FilterResponseType]
    METRIC: _ClassVar[FilterResponseType]
NONE: FilterResponseType
CONFIG: FilterResponseType
RESOURCE: FilterResponseType
BUNDLE: FilterResponseType
TASK: FilterResponseType
METRIC: FilterResponseType

class Empty(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class VersionResponse(_message.Message):
    __slots__ = ("version",)
    VERSION_FIELD_NUMBER: _ClassVar[int]
    version: str
    def __init__(self, version: _Optional[str] = ...) -> None: ...

class FilterRequest(_message.Message):
    __slots__ = ("addr",)
    ADDR_FIELD_NUMBER: _ClassVar[int]
    addr: str
    def __init__(self, addr: _Optional[str] = ...) -> None: ...

class FilterResponse(_message.Message):
    __slots__ = ("bitmask",)
    BITMASK_FIELD_NUMBER: _ClassVar[int]
    bitmask: int
    def __init__(self, bitmask: _Optional[int] = ...) -> None: ...

class ListRequest(_message.Message):
    __slots__ = ("subpath",)
    SUBPATH_FIELD_NUMBER: _ClassVar[int]
    subpath: str
    def __init__(self, subpath: _Optional[str] = ...) -> None: ...

class ListResponse(_message.Message):
    __slots__ = ("addrs",)
    ADDRS_FIELD_NUMBER: _ClassVar[int]
    addrs: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, addrs: _Optional[_Iterable[str]] = ...) -> None: ...

class GetRequest(_message.Message):
    __slots__ = ("addr",)
    ADDR_FIELD_NUMBER: _ClassVar[int]
    addr: str
    def __init__(self, addr: _Optional[str] = ...) -> None: ...

class GetResponse(_message.Message):
    __slots__ = ("exists", "resource_definition", "outputs")
    class OutputsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    EXISTS_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_DEFINITION_FIELD_NUMBER: _ClassVar[int]
    OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    exists: bool
    resource_definition: bytes
    outputs: _containers.ScalarMap[str, str]
    def __init__(self, exists: bool = ..., resource_definition: _Optional[bytes] = ..., outputs: _Optional[_Mapping[str, str]] = ...) -> None: ...

class PlanRequest(_message.Message):
    __slots__ = ("addr", "current", "desired")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    CURRENT_FIELD_NUMBER: _ClassVar[int]
    DESIRED_FIELD_NUMBER: _ClassVar[int]
    addr: str
    current: bytes
    desired: bytes
    def __init__(self, addr: _Optional[str] = ..., current: _Optional[bytes] = ..., desired: _Optional[bytes] = ...) -> None: ...

class PlanResponseElement(_message.Message):
    __slots__ = ("op_definition", "writes_outputs", "friendly_message")
    OP_DEFINITION_FIELD_NUMBER: _ClassVar[int]
    WRITES_OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    FRIENDLY_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    op_definition: str
    writes_outputs: _containers.RepeatedScalarFieldContainer[str]
    friendly_message: str
    def __init__(self, op_definition: _Optional[str] = ..., writes_outputs: _Optional[_Iterable[str]] = ..., friendly_message: _Optional[str] = ...) -> None: ...

class PlanResponse(_message.Message):
    __slots__ = ("ops",)
    OPS_FIELD_NUMBER: _ClassVar[int]
    ops: _containers.RepeatedCompositeFieldContainer[PlanResponseElement]
    def __init__(self, ops: _Optional[_Iterable[_Union[PlanResponseElement, _Mapping]]] = ...) -> None: ...

class OpExecRequest(_message.Message):
    __slots__ = ("addr", "op")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    OP_FIELD_NUMBER: _ClassVar[int]
    addr: str
    op: str
    def __init__(self, addr: _Optional[str] = ..., op: _Optional[str] = ...) -> None: ...

class OpExecResponse(_message.Message):
    __slots__ = ("outputs", "friendly_message")
    class OutputsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    FRIENDLY_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    outputs: _containers.ScalarMap[str, str]
    friendly_message: str
    def __init__(self, outputs: _Optional[_Mapping[str, str]] = ..., friendly_message: _Optional[str] = ...) -> None: ...

class AddrPhyToVirtRequest(_message.Message):
    __slots__ = ("addr",)
    ADDR_FIELD_NUMBER: _ClassVar[int]
    addr: str
    def __init__(self, addr: _Optional[str] = ...) -> None: ...

class AddrPhyToVirtResponse(_message.Message):
    __slots__ = ("has_virt", "virt_addr")
    HAS_VIRT_FIELD_NUMBER: _ClassVar[int]
    VIRT_ADDR_FIELD_NUMBER: _ClassVar[int]
    has_virt: bool
    virt_addr: str
    def __init__(self, has_virt: bool = ..., virt_addr: _Optional[str] = ...) -> None: ...

class AddrVirtToPhyRequest(_message.Message):
    __slots__ = ("addr",)
    ADDR_FIELD_NUMBER: _ClassVar[int]
    addr: str
    def __init__(self, addr: _Optional[str] = ...) -> None: ...

class ReadOutput(_message.Message):
    __slots__ = ("addr", "key")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    addr: str
    key: str
    def __init__(self, addr: _Optional[str] = ..., key: _Optional[str] = ...) -> None: ...

class Deferred(_message.Message):
    __slots__ = ("reads",)
    READS_FIELD_NUMBER: _ClassVar[int]
    reads: _containers.RepeatedCompositeFieldContainer[ReadOutput]
    def __init__(self, reads: _Optional[_Iterable[_Union[ReadOutput, _Mapping]]] = ...) -> None: ...

class Path(_message.Message):
    __slots__ = ("path",)
    PATH_FIELD_NUMBER: _ClassVar[int]
    path: str
    def __init__(self, path: _Optional[str] = ...) -> None: ...

class AddrVirtToPhyResponse(_message.Message):
    __slots__ = ("not_present", "deferred", "present", "null")
    NOT_PRESENT_FIELD_NUMBER: _ClassVar[int]
    DEFERRED_FIELD_NUMBER: _ClassVar[int]
    PRESENT_FIELD_NUMBER: _ClassVar[int]
    NULL_FIELD_NUMBER: _ClassVar[int]
    not_present: Empty
    deferred: Deferred
    present: Path
    null: Path
    def __init__(self, not_present: _Optional[_Union[Empty, _Mapping]] = ..., deferred: _Optional[_Union[Deferred, _Mapping]] = ..., present: _Optional[_Union[Path, _Mapping]] = ..., null: _Optional[_Union[Path, _Mapping]] = ...) -> None: ...

class SubpathsResponse(_message.Message):
    __slots__ = ("subpaths",)
    SUBPATHS_FIELD_NUMBER: _ClassVar[int]
    subpaths: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, subpaths: _Optional[_Iterable[str]] = ...) -> None: ...

class Skeleton(_message.Message):
    __slots__ = ("addr", "body")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    BODY_FIELD_NUMBER: _ClassVar[int]
    addr: str
    body: bytes
    def __init__(self, addr: _Optional[str] = ..., body: _Optional[bytes] = ...) -> None: ...

class GetSkeletonsResponse(_message.Message):
    __slots__ = ("skeletons",)
    SKELETONS_FIELD_NUMBER: _ClassVar[int]
    skeletons: _containers.RepeatedCompositeFieldContainer[Skeleton]
    def __init__(self, skeletons: _Optional[_Iterable[_Union[Skeleton, _Mapping]]] = ...) -> None: ...

class StructIdent(_message.Message):
    __slots__ = ("name",)
    NAME_FIELD_NUMBER: _ClassVar[int]
    name: str
    def __init__(self, name: _Optional[str] = ...) -> None: ...

class FieldIdent(_message.Message):
    __slots__ = ("parent", "name")
    PARENT_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    parent: str
    name: str
    def __init__(self, parent: _Optional[str] = ..., name: _Optional[str] = ...) -> None: ...

class EnumVariantIdent(_message.Message):
    __slots__ = ("parent", "name")
    PARENT_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    parent: str
    name: str
    def __init__(self, parent: _Optional[str] = ..., name: _Optional[str] = ...) -> None: ...

class DocIdent(_message.Message):
    __slots__ = ("struct", "field", "enum_variant")
    STRUCT_FIELD_NUMBER: _ClassVar[int]
    FIELD_FIELD_NUMBER: _ClassVar[int]
    ENUM_VARIANT_FIELD_NUMBER: _ClassVar[int]
    struct: StructIdent
    field: FieldIdent
    enum_variant: EnumVariantIdent
    def __init__(self, struct: _Optional[_Union[StructIdent, _Mapping]] = ..., field: _Optional[_Union[FieldIdent, _Mapping]] = ..., enum_variant: _Optional[_Union[EnumVariantIdent, _Mapping]] = ...) -> None: ...

class GetDocRequest(_message.Message):
    __slots__ = ("addr", "ident")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    IDENT_FIELD_NUMBER: _ClassVar[int]
    addr: str
    ident: DocIdent
    def __init__(self, addr: _Optional[str] = ..., ident: _Optional[_Union[DocIdent, _Mapping]] = ...) -> None: ...

class GetDocResponse(_message.Message):
    __slots__ = ("has_doc", "type", "markdown", "fields")
    HAS_DOC_FIELD_NUMBER: _ClassVar[int]
    TYPE_FIELD_NUMBER: _ClassVar[int]
    MARKDOWN_FIELD_NUMBER: _ClassVar[int]
    FIELDS_FIELD_NUMBER: _ClassVar[int]
    has_doc: bool
    type: str
    markdown: str
    fields: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, has_doc: bool = ..., type: _Optional[str] = ..., markdown: _Optional[str] = ..., fields: _Optional[_Iterable[str]] = ...) -> None: ...

class EqRequest(_message.Message):
    __slots__ = ("addr", "a", "b")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    A_FIELD_NUMBER: _ClassVar[int]
    B_FIELD_NUMBER: _ClassVar[int]
    addr: str
    a: bytes
    b: bytes
    def __init__(self, addr: _Optional[str] = ..., a: _Optional[bytes] = ..., b: _Optional[bytes] = ...) -> None: ...

class EqResponse(_message.Message):
    __slots__ = ("equal",)
    EQUAL_FIELD_NUMBER: _ClassVar[int]
    equal: bool
    def __init__(self, equal: bool = ...) -> None: ...

class DiagnosticPosition(_message.Message):
    __slots__ = ("line", "col")
    LINE_FIELD_NUMBER: _ClassVar[int]
    COL_FIELD_NUMBER: _ClassVar[int]
    line: int
    col: int
    def __init__(self, line: _Optional[int] = ..., col: _Optional[int] = ...) -> None: ...

class DiagnosticSpan(_message.Message):
    __slots__ = ("start", "end")
    START_FIELD_NUMBER: _ClassVar[int]
    END_FIELD_NUMBER: _ClassVar[int]
    start: DiagnosticPosition
    end: DiagnosticPosition
    def __init__(self, start: _Optional[_Union[DiagnosticPosition, _Mapping]] = ..., end: _Optional[_Union[DiagnosticPosition, _Mapping]] = ...) -> None: ...

class Diagnostic(_message.Message):
    __slots__ = ("severity", "span", "message")
    SEVERITY_FIELD_NUMBER: _ClassVar[int]
    SPAN_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    severity: int
    span: DiagnosticSpan
    message: str
    def __init__(self, severity: _Optional[int] = ..., span: _Optional[_Union[DiagnosticSpan, _Mapping]] = ..., message: _Optional[str] = ...) -> None: ...

class DiagRequest(_message.Message):
    __slots__ = ("addr", "a")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    A_FIELD_NUMBER: _ClassVar[int]
    addr: str
    a: bytes
    def __init__(self, addr: _Optional[str] = ..., a: _Optional[bytes] = ...) -> None: ...

class DiagResponse(_message.Message):
    __slots__ = ("diagnostics",)
    DIAGNOSTICS_FIELD_NUMBER: _ClassVar[int]
    diagnostics: _containers.RepeatedCompositeFieldContainer[Diagnostic]
    def __init__(self, diagnostics: _Optional[_Iterable[_Union[Diagnostic, _Mapping]]] = ...) -> None: ...

class TaskExecRequest(_message.Message):
    __slots__ = ("addr", "body", "arg", "state")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    BODY_FIELD_NUMBER: _ClassVar[int]
    ARG_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    addr: str
    body: bytes
    arg: bytes
    state: bytes
    def __init__(self, addr: _Optional[str] = ..., body: _Optional[bytes] = ..., arg: _Optional[bytes] = ..., state: _Optional[bytes] = ...) -> None: ...

class TaskExecResponse(_message.Message):
    __slots__ = ("next_state", "modified_files", "outputs", "secrets", "friendly_message", "delay_until")
    class OutputsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    class SecretsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    NEXT_STATE_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_FILES_FIELD_NUMBER: _ClassVar[int]
    OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    SECRETS_FIELD_NUMBER: _ClassVar[int]
    FRIENDLY_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    DELAY_UNTIL_FIELD_NUMBER: _ClassVar[int]
    next_state: bytes
    modified_files: _containers.RepeatedScalarFieldContainer[str]
    outputs: _containers.ScalarMap[str, str]
    secrets: _containers.ScalarMap[str, str]
    friendly_message: str
    delay_until: int
    def __init__(self, next_state: _Optional[bytes] = ..., modified_files: _Optional[_Iterable[str]] = ..., outputs: _Optional[_Mapping[str, str]] = ..., secrets: _Optional[_Mapping[str, str]] = ..., friendly_message: _Optional[str] = ..., delay_until: _Optional[int] = ...) -> None: ...

class UnbundleRequest(_message.Message):
    __slots__ = ("addr", "bundle")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    BUNDLE_FIELD_NUMBER: _ClassVar[int]
    addr: str
    bundle: bytes
    def __init__(self, addr: _Optional[str] = ..., bundle: _Optional[bytes] = ...) -> None: ...

class UnbundleResponseElement(_message.Message):
    __slots__ = ("addr", "contents")
    ADDR_FIELD_NUMBER: _ClassVar[int]
    CONTENTS_FIELD_NUMBER: _ClassVar[int]
    addr: str
    contents: bytes
    def __init__(self, addr: _Optional[str] = ..., contents: _Optional[bytes] = ...) -> None: ...

class UnbundleResponse(_message.Message):
    __slots__ = ("bundles",)
    BUNDLES_FIELD_NUMBER: _ClassVar[int]
    bundles: _containers.RepeatedCompositeFieldContainer[UnbundleResponseElement]
    def __init__(self, bundles: _Optional[_Iterable[_Union[UnbundleResponseElement, _Mapping]]] = ...) -> None: ...
