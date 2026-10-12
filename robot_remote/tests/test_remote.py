"""通过实际本机 HTTP、WebSocket 和 gRPC 验证网页接入语义。"""
import asyncio
from concurrent import futures
import io
import unittest

from aiohttp import ClientTimeout, WSMsgType
from aiohttp.test_utils import TestClient, TestServer
import grpc

from robotd.control import Controller
from robotd.pb import robot_pb2_grpc as pg
from robotd.server import Service
from robotd.store import Store
from robot_remote.server import GATEWAY, create_app


class RemoteTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.controller = Controller(Store(":memory:"), simulated=True)
        self.controller.start()
        self.server = grpc.server(futures.ThreadPoolExecutor(max_workers=8))
        pg.add_RobotControlServicer_to_server(Service(self.controller, "upstream-secret"),
                                            self.server)
        self.port = self.server.add_insecure_port("127.0.0.1:0")
        self.server.start()
        app = create_app(f"127.0.0.1:{self.port}", robotd_token="upstream-secret",
                         api_token="web-secret", allowed_origins=["http://localhost:5173"],
                         state_interval_ms=50, rpc_timeout=0.5)
        self.client = TestClient(TestServer(app), timeout=ClientTimeout(total=3))
        await self.client.start_server()
        self.headers = {"Authorization": "Bearer web-secret"}

    async def asyncTearDown(self):
        await self.client.close()
        await asyncio.to_thread(self.server.stop(0).wait, 2)
        await asyncio.to_thread(self.controller.close)

    async def api(self, method, path, params=None, status=200, headers=None):
        async with self.client.request(method, path, json=params,
                                       headers=self.headers if headers is None else headers) as reply:
            payload = await reply.json()
            self.assertEqual(reply.status, status, payload)
            return payload

    async def acquire(self, ttl=1000):
        result = await self.api("POST", "/api/control/acquire", {"owner": "http", "ttl_ms": ttl})
        self.assertNotIn("token", result["result"])
        return result["result"]["session_id"]

    async def ws_receive(self, ws, predicate):
        async with asyncio.timeout(3):
            while True:
                frame = await ws.receive()
                self.assertEqual(frame.type, WSMsgType.TEXT, frame)
                message = frame.json()
                if predicate(message):
                    return message

    async def ws_command(self, ws, request_id, method, params=None):
        await ws.send_json({"id": request_id, "method": method, "params": params or {}})
        return await self.ws_receive(ws, lambda message: message.get("type") == "reply"
                                     and message.get("id") == request_id)

    async def websocket(self):
        ws = await self.client.ws_connect("/ws", headers={"Origin": "http://localhost:5173"})
        ready = await ws.receive_json(timeout=2)
        self.assertFalse(ready["authenticated"])
        auth = await self.ws_command(ws, 1, "auth", {"token": "web-secret"})
        self.assertTrue(auth["result"]["authenticated"])
        return ws

    async def state_until(self, predicate):
        async with asyncio.timeout(3):
            while True:
                state = (await self.api("GET", "/api/state"))["result"]
                if predicate(state):
                    return state
                await asyncio.sleep(0.02)

    async def test_http_config_control_validation_and_stop(self):
        self.assertTrue((await self.api("GET", "/healthz", headers={}))["ok"])
        snapshot = (await self.api("GET", "/api/config"))["result"]
        snapshot["config"]["metadata"]["name"] = "网页机器人"
        params = {"expected_revision": snapshot["revision"], "config": snapshot["config"]}
        updated = await self.api("PUT", "/api/config", params)
        self.assertEqual(updated["result"]["revision"], "2")
        conflict = await self.api("PUT", "/api/config", params, status=409)
        self.assertEqual(conflict["error"]["code"], "ABORTED")
        session_id = await self.acquire()
        occupied = await self.api("POST", "/api/control/acquire", {}, status=429)
        self.assertEqual(occupied["error"]["code"], "RESOURCE_EXHAUSTED")
        await self.api("POST", "/api/targets", {"session_id": session_id,
                       "targets": {"neck": {"yaw_rad": 0.1},
                                   "chassis": {"speed_m_s": 0.1, "ttl_ms": 100}}})
        state = await self.state_until(lambda state: not state["stopped"])
        self.assertEqual(state["owner"], "http")
        bad = await self.api("POST", "/api/targets", {"session_id": session_id,
                            "targets": {"neck": {"yaw_rad": 10}}}, status=400)
        self.assertEqual(bad["error"]["code"], "INVALID_ARGUMENT")
        await self.api("POST", "/api/control/renew", {"session_id": session_id})
        await self.api("POST", "/api/stop", {"latch": True})
        await self.state_until(lambda state: state["stopped"] and state["latched"])
        stale = await self.api("POST", "/api/control/renew", {"session_id": session_id}, status=403)
        self.assertEqual(stale["error"]["code"], "PERMISSION_DENIED")
        await self.api("POST", "/api/control/acquire", {}, status=409)
        new = (await self.api("POST", "/api/control/acquire", {"clear_latch": True}))["result"]
        await self.api("POST", "/api/neck/zero", {"session_id": new["session_id"], "mask": 3})
        await self.api("POST", "/api/control/release", {"session_id": new["session_id"]})
        await self.state_until(lambda state: state["stopped"] and not state["owner"])

    async def test_websocket_push_disconnect_and_session_binding(self):
        ws = await self.websocket()
        state = await self.ws_receive(ws, lambda message: message["type"] == "state")
        self.assertTrue(state["state"]["simulated"])
        lease = (await self.ws_command(ws, 2, "acquire", {"owner": "browser"}))["result"]
        self.assertNotIn("token", lease)
        await self.api("POST", "/api/control/renew", {"session_id": lease["session_id"]}, status=403)
        other = await self.websocket()
        denied = await self.ws_command(other, 2, "renew", {"session_id": lease["session_id"]})
        self.assertEqual(denied["error"]["code"], "PERMISSION_DENIED")
        await other.close()
        reply = await self.ws_command(ws, 3, "targets", {"targets": {"neck": {"yaw_rad": 0.1}}})
        self.assertIn("result", reply)
        await self.ws_receive(ws, lambda message: message["type"] == "state"
                              and message["state"]["owner"] == "browser"
                              and not message["state"]["stopped"])
        await ws.close()
        await self.state_until(lambda state: state["stopped"] and not state["owner"])
        self.assertFalse(self.client.server.app[GATEWAY].sessions)

    async def test_lease_expiry_and_chassis_ttl_are_independent(self):
        session_id = await self.acquire(300)
        await self.api("POST", "/api/targets", {"session_id": session_id,
                       "targets": {"chassis": {"speed_m_s": 0.1, "ttl_ms": 100}}})
        await asyncio.sleep(0.13)
        await self.api("POST", "/api/control/renew", {"session_id": session_id})
        # 浏览器仍续期控制权，但不能因此延长之前的底盘速度。
        self.assertIsNone(self.controller.chassis_target)
        self.assertIsNotNone(self.controller.lease)
        await self.state_until(lambda state: not state["owner"] and state["stopped"])
        await self.api("POST", "/api/control/renew", {"session_id": session_id}, status=403)
        ws = await self.websocket()
        await self.ws_command(ws, 2, "acquire", {"ttl_ms": 200})
        await self.ws_command(ws, 3, "targets", {"targets": {"neck": {"yaw_rad": 0.1}}})
        # WebSocket ping/pong 和状态推送不能续期控制权。
        await self.ws_receive(ws, lambda message: message["type"] == "state"
                              and message["state"]["stopped"] and not message["state"]["owner"])
        expired = await self.ws_command(ws, 4, "targets", {"targets": {"neck": {"yaw_rad": 0}}})
        self.assertIn(expired["error"]["code"], ("INVALID_ARGUMENT", "PERMISSION_DENIED"))
        await ws.close()

    async def test_auth_origin_malformed_json_and_preflight(self):
        await self.api("GET", "/api/state", status=401, headers={})
        await self.api("GET", "/api/state", status=403,
                       headers={**self.headers, "Origin": "https://other.example"})
        headers = {**self.headers, "Origin": "http://localhost:5173"}
        async with self.client.options("/api/targets", headers=headers) as reply:
            self.assertEqual(reply.status, 204)
            self.assertEqual(reply.headers["Access-Control-Allow-Origin"], "http://localhost:5173")
        await self.api("GET", "/api/state", headers=headers)
        for value in ('[]', '{bad', '{"ttl_ms":NaN}', '{"typo":1}'):
            async with self.client.post("/api/control/acquire", data=value,
                                       headers={**self.headers, "Content-Type": "application/json"}) as reply:
                self.assertEqual(reply.status, 400, await reply.text())
        async with self.client.post("/api/stop", data="{}", headers=self.headers) as reply:
            self.assertEqual(reply.status, 415)
        async with self.client.post("/api/stop", data=io.BytesIO(b"x" * (1024 * 1024 + 1)),
                                   headers={**self.headers, "Content-Type": "application/json"}) as reply:
            self.assertEqual(reply.status, 413)
        ws = await self.client.ws_connect("/ws")
        ready = await ws.receive_json(timeout=2)
        self.assertFalse(ready["authenticated"])
        invalid = await self.ws_command(ws, 1, "get_state")
        self.assertEqual(invalid["error"]["code"], "UNAUTHENTICATED")
        await ws.close()
        ws = await self.websocket()
        await ws.send_str("[]")
        invalid = await self.ws_receive(ws, lambda message: message["type"] == "reply")
        self.assertEqual(invalid["error"]["code"], "INVALID_ARGUMENT")
        unknown = await self.ws_command(ws, 3, "missing")
        self.assertEqual(unknown["error"]["code"], "NOT_FOUND")
        await ws.close()

    async def test_actions_and_gateway_shutdown_release(self):
        action = {"name": "wave", "duration_ms": 1000,
                  "steps": [{"at_ms": 0, "targets": {"neck": {"yaw_rad": 0.1}}}]}
        saved = await self.api("POST", "/api/actions", {"action": action, "expected_revision": 0})
        self.assertEqual(saved["result"]["revision"], "1")
        actions = (await self.api("GET", "/api/actions"))["result"]["actions"]
        self.assertEqual(actions[0]["action"]["name"], "wave")
        session_id = await self.acquire()
        await self.api("POST", "/api/actions/run", {"session_id": session_id, "name": "wave"})
        await self.state_until(lambda state: state["action"] == "wave")
        await self.api("POST", "/api/actions/cancel", {"session_id": session_id})
        await self.api("DELETE", "/api/actions", {"name": "wave", "expected_revision": 1})
        await self.api("POST", "/api/targets", {"session_id": session_id,
                       "targets": {"neck": {"yaw_rad": 0.1}}})
        await self.client.close()
        self.assertIsNone(self.controller.lease)
        self.assertTrue(self.controller.stopped)

    async def test_robotd_failure_reports_error_without_replay(self):
        ws = await self.websocket()
        lease = await self.ws_command(ws, 2, "acquire")
        self.assertIn("result", lease)
        await self.ws_command(ws, 3, "targets", {"targets": {"neck": {"yaw_rad": 0.1}}})
        await asyncio.to_thread(self.server.stop(0).wait, 2)
        error = await self.ws_receive(ws, lambda message: message["type"] == "backend_error")
        self.assertIn(error["error"]["code"], ("UNAVAILABLE", "DEADLINE_EXCEEDED"))
        for path in ("/api/state", "/healthz"):
            async with self.client.get(path, headers=self.headers) as reply:
                self.assertIn(reply.status, (503, 504))
                self.assertIn((await reply.json())["error"]["code"],
                              ("UNAVAILABLE", "DEADLINE_EXCEEDED"))
        await asyncio.to_thread(self.controller.close)
        self.controller = Controller(Store(":memory:"), simulated=True)
        self.controller.start()
        self.server = grpc.server(futures.ThreadPoolExecutor(max_workers=8))
        pg.add_RobotControlServicer_to_server(Service(self.controller, "upstream-secret"),
                                            self.server)
        self.assertEqual(self.server.add_insecure_port(f"127.0.0.1:{self.port}"), self.port)
        self.server.start()
        recovered = await self.ws_receive(ws, lambda message: message["type"] == "state")
        self.assertTrue(recovered["state"]["stopped"])
        self.assertFalse(recovered["state"]["owner"])
        stale = await self.ws_command(ws, 4, "targets", {"targets": {"neck": {"yaw_rad": 0.1}}})
        self.assertIn("error", stale)
        lease = await self.ws_command(ws, 5, "acquire")
        self.assertIn("result", lease)
        await ws.close()


if __name__ == "__main__":
    unittest.main()
