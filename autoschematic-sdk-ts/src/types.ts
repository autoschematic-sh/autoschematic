// src/types.ts
export type FilterOutput = 'CONFIG' | 'RESOURCE' | 'BUNDLE' | 'NONE';

export interface ConnectorConstructor {
    // ndew(name: string, prefix: string): unknown;
    __new(name: string, prefix: string): Promise<Connector>;
}

export interface Connector {
    init(): Promise<void>;
    filter(addr: string): Promise<FilterOutput>;
    list(subpath: string): Promise<string[]>;
    subpaths(): Promise<string[]>;
    get(addr: string): Promise<{
        exists: boolean;
        resourceDefinition?: Uint8Array;
        outputs?: Record<string, string>;
    }>;
    plan(
        addr: string,
        current?: Uint8Array,
        desired?: Uint8Array
    ): Promise<Array<{
        opDefinition: string;
        writesOutputs: string[];
        friendlyMessage?: string;
    }>>;
    opExec(
        addr: string,
        op: string
    ): Promise<{ outputs?: Record<string, string>; friendlyMessage?: string }>;
    addrVirtToPhy(addr: string):
        Promise<
            | { kind: 'NotPresent' }
            | { kind: 'Deferred'; reads: Array<{ addr: string; key: string }> }
            | { kind: 'Present'; path: string }
            | { kind: 'Null'; path: string }
        >;
    addrPhyToVirt(addr: string): Promise<string | null>;
    getSkeletons(): Promise<Array<{ addr: string; body: Uint8Array }>>;
    getDocstring(
        addr: string,
        ident: { struct?: string; field?: { parent: string; name: string } }
    ): Promise<string | null>;
    eq(addr: string, a: Uint8Array, b: Uint8Array): Promise<boolean>;
    diag(addr: string, a: Uint8Array): Promise<Array<{
        severity: number;
        span: {
            start: { line: number; col: number };
            end: { line: number; col: number };
        };
        message: string;
    }>>;
    unbundle(
        addr: string,
        bundle: Uint8Array
    ): Promise<Array<{ filename: string; fileContents: string }>>;
}
