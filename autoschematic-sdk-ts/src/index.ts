import { createConnectorServer } from "./grpcServer";
import { Connector, ConnectorConstructor } from "./types";
import { matchAddr } from "./addr";

export default async function connectorMain(constructor: ConnectorConstructor) {
    let name = process.argv[2];
    let prefix = process.argv[3]; 
    let socket = process.argv[4]; 
    let error_dump = process.argv[5]; 
    
    let connector = await constructor.__new(name, prefix);
    
    
    let server = await createConnectorServer(connector, socket);
}