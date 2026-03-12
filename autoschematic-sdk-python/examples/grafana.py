"""
A Grafana connector for autoschematic that manages datasources and dashboards
via the Grafana HTTP API.

Requires: aiohttp (pip install aiohttp)

Environment variables:
  GRAFANA_URL     - Base URL of the Grafana instance (e.g. http://localhost:3000)
  GRAFANA_API_KEY - API key or service account token
"""

from dataclasses import dataclass
import json
import os

import aiohttp

from autoschematic_sdk import (
    Connector,
    FilterResponse,
    GetResponse,
    PlanResponseElement,
    OpExecResponse,
    VirtToPhyDeferred,
    VirtToPhyNotPresent,
    VirtToPhyNull,
    VirtToPhyPresent,
    connector_main,
    match_addr,
    InvalidAddr,
)

DATASOURCE_ADDR = match_addr("grafana/datasources/[name].json")
DASHBOARD_ADDR = match_addr("grafana/dashboards/[uid].json")

@dataclass
class DatasourceAddress:
    name: str

@dataclass
class DashboardAddress:
    uid: str

def decode_addr(addr: str) -> DatasourceAddress | DashboardAddress | None:
    match addr.split("/"):
        case ["grafana", "datasources", name] if name.endswith(".json"):
            return DatasourceAddress(name=name.rstrip(".json"))
        case ["grafana", "dashboards", uid] if uid.endswith(".json"):
            return DashboardAddress(uid=uid.rstrip(".json"))
        case _:
            return None
            

# "Transient fields" are those we strip from GET responses (they're not user-editable)
DATASOURCE_TRANSIENT_FIELDS = {"id", "orgId", "uid", "typeLogoUrl", "readOnly"}
DASHBOARD_TRANSIENT_FIELDS = {"id", "version"}


class GrafanaConnector(Connector):
    def __init__(self, name: str, prefix: str) -> None:
        self.name = name
        self.prefix = prefix
        self.base_url: str = ""
        self.session: aiohttp.ClientSession | None = None

    async def init(self) -> None:
        self.base_url = os.environ.get("GRAFANA_URL", "").rstrip("/")
        api_key = os.environ.get("GRAFANA_API_KEY", "")

        if not self.base_url:
            raise RuntimeError("GRAFANA_URL environment variable is required")
        if not api_key:
            raise RuntimeError("GRAFANA_API_KEY environment variable is required")

        self.session = aiohttp.ClientSession(
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json",
            },
        )


    # API helpers
    async def _api_get(self, path: str) -> tuple[int, dict | list]:
        assert self.session is not None
        async with self.session.get(f"{self.base_url}{path}") as resp:
            body = await resp.json()
            return resp.status, body

    async def _api_post(self, path: str, payload: dict) -> tuple[int, dict]:
        assert self.session is not None
        async with self.session.post(f"{self.base_url}{path}", json=payload) as resp:
            body = await resp.json()
            return resp.status, body

    async def _api_put(self, path: str, payload: dict) -> tuple[int, dict]:
        assert self.session is not None
        async with self.session.put(f"{self.base_url}{path}", json=payload) as resp:
            body = await resp.json()
            return resp.status, body

    async def _api_delete(self, path: str) -> tuple[int, dict]:
        assert self.session is not None
        async with self.session.delete(f"{self.base_url}{path}") as resp:
            body = await resp.json()
            return resp.status, body


    async def filter(self, addr: str) -> FilterResponse:
        if decode_addr(addr) is not None:
            return FilterResponse.RESOURCE
        else:
            return FilterResponse.NONE

    async def list(self, subpath: str) -> list[str]:
        addrs: list[str] = []

        # enumerate datasources (TODO: paginate this!)
        status, datasources = await self._api_get("/api/datasources")
        if status == 200 and isinstance(datasources, list):
            for ds in datasources:
                name = ds.get("name", "")
                if name:
                    addrs.append(f"grafana/datasources/{name}.json")

        # enumerate dashboards...
        status, results = await self._api_get("/api/search?type=dash-db")
        if status == 200 and isinstance(results, list):
            for db in results:
                uid = db.get("uid", "")
                if uid:
                    addrs.append(f"grafana/dashboards/{uid}.json")

        return addrs

    async def get(self, addr: str) -> GetResponse:
        match decode_addr(addr):
            case DashboardAddress(uid):
                return await self._get_dashboard(uid)

            case DatasourceAddress(name):
                return await self._get_datasource(name)
            case _:
                return GetResponse(exists=False)

    async def _get_datasource(self, name: str) -> GetResponse:
        status, body = await self._api_get(f"/api/datasources/name/{name}")

        match status:
            case 404:
                return GetResponse(exists=False)
            case 200:
                assert isinstance(body, dict)

                ds_id = str(body.get("id", ""))
                for field in DATASOURCE_TRANSIENT_FIELDS:
                    body.pop(field, None)

                return GetResponse(
                    exists=True,
                    resource_definition=json.dumps(body, indent=2).encode(),
                    outputs={"id": ds_id},
                )
                
            case _:
                raise RuntimeError(f"Failed to get datasource '{name}': {status} {body}")
                

    async def _get_dashboard(self, uid: str) -> GetResponse:
        status, body = await self._api_get(f"/api/dashboards/uid/{uid}")
        match status:
            case 404:
                return GetResponse(exists=False)
            case 200:
                assert(isinstance(body, dict))

                dashboard = body.get("dashboard", {})
                meta = body.get("meta", {})

                db_id = str(dashboard.get("id", ""))
                version = str(dashboard.get("version", ""))

                title = str(dashboard.get("title", ""))

                # Remove non user-editable fields
                for field in DASHBOARD_TRANSIENT_FIELDS:
                    dashboard.pop(field, None)

                return GetResponse(
                    exists=True,
                    resource_definition=json.dumps(dashboard, indent=2).encode(),
                    # virt_addr=f"grafana/dashboards/{title}.json",
                    outputs={"id": db_id, "version": version, "folder_id": str(meta.get("folderId", ""))},
                )
            case _:
                raise RuntimeError(f"Failed to get dashboard '{uid}': {status} {body}")

    async def plan(
        self,
        addr: str,
        current: bytes | None,
        desired: bytes | None,
    ) -> list[PlanResponseElement]:
        resource_type = None

        match decode_addr(addr):
            case DatasourceAddress(name):
                resource_type = "datasource"
            case DashboardAddress(uid):
                resource_type = "dashboard"
            case _:
                raise InvalidAddr(addr)

        match [current, desired]:
            case [None, None]:
                pass
            case [_, None]:
                return [
                    PlanResponseElement(
                        op_definition=json.dumps({"action": "delete", "type": resource_type}),
                        writes_outputs=[],
                        friendly_message=f"Delete {resource_type}",
                    )
                ]
            case [None, desired]:
                assert desired is not None
                return [
                    PlanResponseElement(
                        op_definition=json.dumps({
                            "action": "create",
                            "body": json.loads(desired),
                        }),
                        writes_outputs=["id"],
                        friendly_message=f"Create {resource_type}",
                    )
                ]
            case [current, desired] if current != desired:
                assert desired is not None
                return [
                    PlanResponseElement(
                        op_definition=json.dumps({
                            "action": "update",
                            "body": json.loads(desired),
                        }),
                        writes_outputs=["id"],
                        friendly_message=f"Update {resource_type}",
                    )
                ]

        return []

    async def op_exec(self, addr: str, op: str) -> OpExecResponse:
        dec_op = json.loads(op)
        action = dec_op["action"]
        # resource_type = dec_op["type"]
        
        match decode_addr(addr):
            case DatasourceAddress(name):
                return await self.exec_datasource(name, action, dec_op.get("body"))
            case DashboardAddress(uid):
                return await self.exec_dashboard(uid, action, dec_op.get("body"))
            case _:
                raise InvalidAddr(addr)

    async def exec_datasource(self, name: str, action: str, body: dict | None) -> OpExecResponse:
        match action:
            case "delete":
                status, resp = await self._api_delete(f"/api/datasources/name/{name}")
                match status:
                    case 200 | 404:
                        return OpExecResponse(
                            outputs={},
                            friendly_message=f"Deleted datasource '{name}'",
                        )
                    case _:
                        raise RuntimeError(f"Failed to delete datasource '{name}': {status} {resp}")

            case "create":
                assert body is not None

                # Merge addr (name) into the post body
                body["name"] = name
                status, resp = await self._api_post("/api/datasources", body)
                match status:
                    case 200 | 201:
                        return OpExecResponse(
                            outputs={"id": str(resp.get("datasource", {}).get("id", resp.get("id", "")))},
                            friendly_message=f"Created datasource '{name}'",
                        )
                    case _:
                        raise RuntimeError(f"Failed to create datasource '{name}': {status} {resp}")

            case "update":
                assert body is not None
                # Fetch the current datasource to get its numeric id for the PUT URL.
                status, current = await self._api_get(f"/api/datasources/name/{name}")
                match status: 
                    case 200:
                        assert isinstance(current, dict)
                        id = current["id"]
                        body["name"] = name
                        body["id"] = id
                        status, resp = await self._api_put(f"/api/datasources/{id}", body)
                        match status: 
                            case 200:
                                return OpExecResponse(
                                    outputs={"id": str(id)},
                                    friendly_message=f"Updated datasource '{name}'",
                                )
                            case _:
                                raise RuntimeError(f"Failed to update datasource '{name}': {status} {resp}")
                    case _:
                        raise RuntimeError(f"Datasource '{name}' not found for update: {status} {current}")

        raise RuntimeError(f"Unknown datasource action: {action}")

    async def exec_dashboard(self, uid: str, action: str, body: dict | None) -> OpExecResponse:
        if action == "delete":
            status, resp = await self._api_delete(f"/api/dashboards/uid/{uid}")
            match status:
                case 200 | 404:
                    return OpExecResponse(
                        outputs={},
                        friendly_message=f"Deleted dashboard '{uid}'",
                    )
                case _:
                    raise RuntimeError(f"Failed to delete dashboard '{uid}': {status} {resp}")

        if action in ("create", "update"):
            assert body is not None
            body["uid"] = uid
            payload = {
                "dashboard": body,
                "overwrite": action == "update",
            }

            status, resp = await self._api_post("/api/dashboards/db", payload)

            match status:
                case 200 | 201:
                    return OpExecResponse(
                        outputs={"id": str(resp.get("id", "")), "version": str(resp.get("version", ""))},
                        friendly_message=f"{'Created' if action == 'create' else 'Updated'} dashboard '{uid}'",
                    )
                case _:
                    raise RuntimeError(f"Failed to {action} dashboard '{uid}': {status} {resp}")

        raise RuntimeError(f"Unknown dashboard action: {action}")


if __name__ == "__main__":
    connector_main(GrafanaConnector)
