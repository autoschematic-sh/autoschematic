import { Server, ServerCredentials } from '@grpc/grpc-js';
import {
    FilterRequest,
    FilterResponse,
    ListRequest,
    ListResponse,
    GetRequest,
    GetResponse,
    PlanRequest,
    PlanResponse,
    OpExecRequest,
    OpExecResponse,
    AddrRequest,
    AddrPhyToVirtResponse,
    SubpathsResponse,
    GetSkeletonsResponse,
    GetDocRequest,
    GetDocResponse,
    EqRequest,
    EqResponse,
    DiagRequest,
    DiagResponse,
    UnbundleRequest,
    UnbundleResponse,
    OpPlanOutput as GrpcOpPlan,
    SkeletonOutput as GrpcSkeleton,
    ReadOutput as GrpcRead,
    Deferred as GrpcDeferred,
    Path as GrpcPath,
    VirtToPhyOutput as GrpcVirtRes,
    BundleOutput as GrpcBundle,
    Empty,
    ConnectorServer,
    ConnectorService,
} from './generated/connector';
import { Connector, FilterOutput } from './types';

export function createConnectorServer(
    impl: Connector,
    socketPath: string
): Server {
    const service: ConnectorServer = {
        init: async (_call, callback) => {
            try {
                await impl.init();
                callback(null, Empty.create());
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        filter: async (call, callback) => {
            try {
                const out = await impl.filter(call.request.addr);
                const idx = ['CONFIG', 'RESOURCE', 'BUNDLE', 'NONE'].indexOf(out);
                callback(null, FilterResponse.create({ filter: idx }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        list: async (call, callback) => {
            try {
                const arr = await impl.list(call.request.subpath);
                callback(null, ListResponse.create({ addrs: arr }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        subpaths: async (_c, cb) => {
            try {
                const arr = await impl.subpaths();
                cb(null, SubpathsResponse.create({ subpaths: arr }));
            } catch (e: any) {
                cb({ code: 13, message: e.message }, null);
            }
        },
        get: async (call, callback) => {
            try {
                const { exists, resourceDefinition, outputs } =
                    await impl.get(call.request.addr)
                        .then(o => o || { exists: false });
                if (!exists) {
                    return callback(null, GetResponse.create({ exists: false }));
                }
                callback(null, GetResponse.create({
                    exists: true,
                    resourceDefinition,
                    outputs: outputs || {},
                }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        plan: async (call, callback) => {
            try {
                const r = call.request as PlanRequest;
                const ops = await impl.plan(
                    r.addr,
                    r.current?.length ? r.current : undefined,
                    r.desired?.length ? r.desired : undefined
                );
                const grpcOps: GrpcOpPlan[] = ops.map(o =>
                    GrpcOpPlan.create({
                        opDefinition: o.opDefinition,
                        writesOutputs: o.writesOutputs,
                        friendlyMessage: o.friendlyMessage ?? '',
                    }));
                callback(null, PlanResponse.create({ ops: grpcOps }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        opExec: async (call, callback) => {
            try {
                const { outputs, friendlyMessage } =
                    await impl.opExec(call.request.addr, call.request.op);
                const map: Record<string, string> = {};
                if (outputs) for (let k in outputs) map[k] = outputs[k]!;
                callback(null, OpExecResponse.create({ outputs: map, friendlyMessage: friendlyMessage ?? '' }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        addrVirtToPhy: async (call, cb) => {
            try {
                const res = await impl.addrVirtToPhy(call.request.addr);
                const msg = GrpcVirtRes.create();
                switch (res.kind) {
                    case 'NotPresent':
                        msg.notPresent = Empty.create();
                        break;
                    case 'Deferred':
                        msg.deferred = GrpcDeferred.create({
                            reads: res.reads.map(r => GrpcRead.create(r)),
                        });
                        break;
                    case 'Present':
                        msg.present = GrpcPath.create({ path: res.path });
                        break;
                    case 'Null':
                        msg.null = GrpcPath.create({ path: res.path });
                        break;
                }
                cb(null, msg);
            } catch (e: any) {
                cb({ code: 13, message: e.message }, null);
            }
        },
        addrPhyToVirt: async (call, callback) => {
            try {
                const v = await impl.addrPhyToVirt(call.request.addr);
                callback(null, AddrPhyToVirtResponse.create({
                    hasVirt: v !== null,
                    virtAddr: v ?? '',
                }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        getSkeletons: async (_c, cb) => {
            try {
                const sk = await impl.getSkeletons();
                const out = sk.map(s =>
                    GrpcSkeleton.create({ addr: s.addr, body: s.body }));
                cb(null, GetSkeletonsResponse.create({ skeletons: out }));
            } catch (e: any) {
                cb({ code: 13, message: e.message }, null);
            }
        },
        getDocstring: async (call, callback) => {
            try {
                const { hasDoc, markdown } =
                    await impl.getDocstring(
                        call.request.addr,
                        // ts-proto gives you a one-of that you unwrap here…
                        call.request.ident!.struct
                            ? { struct: call.request.ident!.struct.name }
                            : {
                                field: {
                                    parent: call.request.ident!.field!.parent,
                                    name: call.request.ident!.field!.name,
                                }
                            }
                    ).then(o => o ? ({ hasDoc: true, markdown: o }) : ({ hasDoc: false, markdown: '' }));
                callback(null, GetDocResponse.create({ hasDoc, markdown }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        eq: async (call, callback) => {
            try {
                const ok = await impl.eq(call.request.addr, call.request.a, call.request.b);
                callback(null, EqResponse.create({ equal: ok }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        diag: async (call, callback) => {
            try {
                const ds = await impl.diag(call.request.addr, call.request.a);
                callback(null, DiagResponse.create({
                    diagnostics: {
                        diagnostics: ds.map(d => ({
                            severity: d.severity,
                            span: {
                                start: d.span.start,
                                end: d.span.end,
                            },
                            message: d.message,
                        }))
                    }
                }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
        unbundle: async (call, callback) => {
            try {
                const bs = await impl.unbundle(call.request.addr, call.request.bundle);
                const out = bs.map(b => GrpcBundle.create({
                    filename: b.filename,
                    fileContents: b.fileContents
                }));
                callback(null, UnbundleResponse.create({ bundles: out }));
            } catch (e: any) {
                callback({ code: 13, message: e.message }, null);
            }
        },
    };

    const server = new Server();
    server.addService(ConnectorService, service);
    server.bindAsync(
        `unix://${socketPath}`,
        ServerCredentials.createInsecure(),
        (err, port) => {
            if (err) throw err;
            server.start();
            console.log(`Connector gRPC server listening on UDS ${socketPath}`);
        }
    );
    return server;
}
