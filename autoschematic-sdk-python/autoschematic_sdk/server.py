from __future__ import annotations

import grpc
from grpc import aio

from .generated import connector_pb2, connector_pb2_grpc
from .types import (
    Connector,
    FieldIdent,
    EnumVariantIdent,
    StructIdent,
    VirtToPhyDeferred,
    VirtToPhyNotPresent,
    VirtToPhyNull,
    VirtToPhyPresent,
)


class _ConnectorServicer(connector_pb2_grpc.ConnectorServicer):
    def __init__(self, impl: Connector) -> None:
        self._impl = impl

    async def Init(self, request, context):
        try:
            await self._impl.init()
            return connector_pb2.Empty()
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Version(self, request, context):
        return connector_pb2.VersionResponse(version="0.13.0")

    async def Filter(self, request, context):
        try:
            result = await self._impl.filter(request.addr)
            return connector_pb2.FilterResponse(bitmask=int(result))
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def List(self, request, context):
        try:
            addrs = await self._impl.list(request.subpath)
            return connector_pb2.ListResponse(addrs=addrs)
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Subpaths(self, request, context):
        try:
            subpaths = await self._impl.subpaths()
            return connector_pb2.SubpathsResponse(subpaths=subpaths)
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Get(self, request, context):
        try:
            res = await self._impl.get(request.addr)
            if not res.exists:
                return connector_pb2.GetResponse(exists=False)
            return connector_pb2.GetResponse(
                exists=True,
                resource_definition=res.resource_definition or b"",
                virt_addr=res.virt_addr or "",
                outputs=res.outputs or {},
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Plan(self, request, context):
        try:
            current = request.current if len(request.current) > 0 else None
            desired = request.desired if len(request.desired) > 0 else None
            ops = await self._impl.plan(request.addr, current, desired)
            return connector_pb2.PlanResponse(
                ops=[
                    connector_pb2.PlanResponseElement(
                        op_definition=op.op_definition,
                        writes_outputs=op.writes_outputs,
                        friendly_message=op.friendly_message,
                    )
                    for op in ops
                ]
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def OpExec(self, request, context):
        try:
            res = await self._impl.op_exec(request.addr, request.op)
            return connector_pb2.OpExecResponse(
                outputs=res.outputs or {},
                friendly_message=res.friendly_message,
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def AddrVirtToPhy(self, request, context):
        try:
            res = await self._impl.addr_virt_to_phy(request.addr)
            resp = connector_pb2.AddrVirtToPhyResponse()
            match res:
                case VirtToPhyNotPresent():
                    resp.not_present.CopyFrom(connector_pb2.Empty())
                case VirtToPhyDeferred(reads=reads):
                    resp.deferred.CopyFrom(
                        connector_pb2.Deferred(
                            reads=[
                                connector_pb2.ReadOutput(addr=r.addr, key=r.key)
                                for r in reads
                            ]
                        )
                    )
                case VirtToPhyPresent(path=path):
                    resp.present.CopyFrom(connector_pb2.Path(path=path))
                case VirtToPhyNull(path=path):
                    resp.null.CopyFrom(connector_pb2.Path(path=path))
            return resp
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def AddrPhyToVirt(self, request, context):
        try:
            virt = await self._impl.addr_phy_to_virt(request.addr)
            return connector_pb2.AddrPhyToVirtResponse(
                has_virt=virt is not None,
                virt_addr=virt or "",
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def GetSkeletons(self, request, context):
        try:
            skeletons = await self._impl.get_skeletons()
            return connector_pb2.GetSkeletonsResponse(
                skeletons=[
                    connector_pb2.Skeleton(addr=s.addr, body=s.body)
                    for s in skeletons
                ]
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def GetDocstring(self, request, context):
        try:
            ident_pb = request.ident
            ident_type = ident_pb.WhichOneof("ident")
            if ident_type == "struct":
                ident = StructIdent(name=ident_pb.struct.name)
            elif ident_type == "field":
                ident = FieldIdent(parent=ident_pb.field.parent, name=ident_pb.field.name)
            elif ident_type == "enum_variant":
                ident = EnumVariantIdent(parent=ident_pb.enum_variant.parent, name=ident_pb.enum_variant.name)
            else:
                ident = StructIdent(name="")

            res = await self._impl.get_docstring(request.addr, ident)
            if res is None:
                return connector_pb2.GetDocResponse(has_doc=False)
            return connector_pb2.GetDocResponse(
                has_doc=res.has_doc,
                type=res.type,
                markdown=res.markdown,
                fields=res.fields,
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Eq(self, request, context):
        try:
            equal = await self._impl.eq(request.addr, request.a, request.b)
            return connector_pb2.EqResponse(equal=equal)
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Diag(self, request, context):
        try:
            diagnostics = await self._impl.diag(request.addr, request.a)
            return connector_pb2.DiagResponse(
                diagnostics=[
                    connector_pb2.Diagnostic(
                        severity=d.severity,
                        span=connector_pb2.DiagnosticSpan(
                            start=connector_pb2.DiagnosticPosition(
                                line=d.span.start.line, col=d.span.start.col
                            ),
                            end=connector_pb2.DiagnosticPosition(
                                line=d.span.end.line, col=d.span.end.col
                            ),
                        ),
                        message=d.message,
                    )
                    for d in diagnostics
                ]
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))

    async def Unbundle(self, request, context):
        try:
            items = await self._impl.unbundle(request.addr, request.bundle)
            return connector_pb2.UnbundleResponse(
                bundles=[
                    connector_pb2.UnbundleResponseElement(
                        addr=item.addr, contents=item.contents
                    )
                    for item in items
                ]
            )
        except Exception as e:
            await context.abort(grpc.StatusCode.INTERNAL, str(e))


async def create_connector_server(
    impl: Connector, socket_path: str
) -> aio.Server:
    server = aio.server()
    connector_pb2_grpc.add_ConnectorServicer_to_server(
        _ConnectorServicer(impl), server
    )
    server.add_insecure_port(f"unix:{socket_path}")
    await server.start()
    return server