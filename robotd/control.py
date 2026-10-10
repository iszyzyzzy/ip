"""一个控制线程拥有所有设备与数据库，RPC 通过有限队列提交工作。"""
from __future__ import annotations

import concurrent.futures
import copy
import itertools
import logging
import queue
import secrets
import threading
import time
import grpc
from google.protobuf.json_format import MessageToDict, ParseDict
from .config import integer
from .drivers import Hardware, Simulated
from .pb import robot_pb2 as pb
from .store import Conflict
from .validation import validate_targets, validate_action

log = logging.getLogger(__name__)


class ControlError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def fail(code, message):
    raise ControlError(code, message)


class Controller:
    def __init__(self, store, simulated=False, driver_factory=None, clock=time.monotonic):
        self.store = store
        self.simulated = simulated
        self.driver_factory = driver_factory or (Simulated if simulated else Hardware)
        self.clock = clock
        self.config_revision, self.config = store.config()
        self.applied_revision = 0
        self.revision = 0
        self.driver = None
        self.lease = None
        self.lease_deadline = 0
        self.chassis_target = None
        self.chassis_deadline = 0
        self.next_chassis = self.next_poll = 0
        self.action = None
        self.action_start = 0
        self.action_index = 0
        self.stopped = True
        self.latched = False
        self.fault = ""
        self.jobs = queue.PriorityQueue()
        self.slots = threading.BoundedSemaphore(64)
        self.counter = itertools.count()
        self.done = threading.Event()
        self.started = threading.Event()
        self.thread = None
        self.state_lock = threading.Lock()
        self.cached_state = pb.RobotState()

    def reconnect(self):
        self.applied_revision = 0
        if self.driver:
            self.driver.close()
        self.driver = self.driver_factory(copy.deepcopy(self.config))
        try:
            self.driver.connect()
            self.applied_revision = self.config_revision
            self.fault = ""
        except Exception as e:
            self.fault = f"设备连接/配置失败：{e}"
            self.latched = True
            log.error(self.fault)

    def start(self):
        self.thread = threading.Thread(target=self._run, name="robot-control", daemon=True)
        self.thread.start()
        if not self.started.wait(10):
            raise TimeoutError("控制线程启动超时")

    def call(self, method, *args, timeout=5, urgent=False):
        if self.done.is_set():
            fail(grpc.StatusCode.UNAVAILABLE, "robotd 已停止")
        reserved = False if urgent else self.slots.acquire(blocking=False)
        if not urgent and not reserved:
            fail(grpc.StatusCode.RESOURCE_EXHAUSTED, "控制请求队列已满")
        future = concurrent.futures.Future()
        self.jobs.put((0 if urgent else 1, next(self.counter), method, args, future, reserved))
        try:
            return future.result(timeout=timeout)
        except concurrent.futures.TimeoutError:
            # 未执行的请求取消；已经执行的请求不能承诺撤销，不自动重发。
            future.cancel()
            fail(grpc.StatusCode.DEADLINE_EXCEEDED, "请求等待超时，请查询状态；不要自动重发")

    def _run(self):
        try:
            self.reconnect()
            self._publish()
            self.started.set()
            while not self.done.is_set():
                due = self.clock() + 0.02 if self.fault else self.next_poll
                if self.chassis_target:
                    due = min(due, self.next_chassis, self.chassis_deadline)
                if self.lease:
                    due = min(due, self.lease_deadline)
                delay = max(0.001, min(0.02, due - self.clock()))
                try:
                    priority, _, method, args, future, reserved = self.jobs.get(timeout=delay)
                except queue.Empty:
                    self.tick()
                    self._publish()
                    continue
                try:
                    if future.set_running_or_notify_cancel():
                        try:
                            # 停止先执行，不能为了刷新遥测再下发一条已到时间的动作。
                            if priority != 0:
                                self.tick()
                            result = getattr(self, method)(*args)
                            self._publish()
                            future.set_result(result)
                        except Exception as e:
                            future.set_exception(e)
                finally:
                    if reserved:
                        self.slots.release()
        except Exception:
            log.exception("控制线程异常退出")
            self.fault = "控制线程异常退出"
            self.latched = True
        finally:
            self.done.set()
            self._stop(revoke=True)
            if self.driver:
                self.driver.close()
            self._publish()
            self.started.set()
            while not self.jobs.empty():
                _, _, _, _, future, reserved = self.jobs.get_nowait()
                if future.set_running_or_notify_cancel():
                    future.set_exception(ControlError(grpc.StatusCode.UNAVAILABLE, "控制线程已停止"))
                if reserved:
                    self.slots.release()

    def close(self):
        self.done.set()
        if self.thread:
            self.thread.join(timeout=10)
            if self.thread.is_alive():
                raise TimeoutError("设备停止尚未完成")
        self.store.close()

    def _publish(self):
        state = pb.RobotState(
            revision=self.revision, config_revision=self.config_revision,
            applied_config_revision=self.applied_revision, simulated=self.simulated,
            stopped=self.stopped, latched=self.latched,
            owner=self.lease.owner if self.lease else "",
            lease_remaining_ms=max(0, int((self.lease_deadline - self.clock()) * 1000))
                if self.lease else 0,
            action=self.action.name if self.action else "", fault=self.fault,
        )
        if self.driver:
            state.devices.update(copy.deepcopy(self.driver.state))
        with self.state_lock:
            self.cached_state = state

    def state(self):
        with self.state_lock:
            out = pb.RobotState()
            out.CopyFrom(self.cached_state)
        return out

    def get_config(self):
        snapshot = pb.ConfigSnapshot(revision=self.config_revision)
        snapshot.config.update(copy.deepcopy(self.config))
        return snapshot

    def update_config(self, request):
        from .config import validate_config
        c = MessageToDict(request.config)
        validate_config(c)
        # 版本冲突不能打断当前动作；确认候选后才停机并提交事务。
        if request.expected_revision != self.config_revision:
            raise Conflict("配置版本不一致")
        for _, value in self.store.actions():
            validate_action(ParseDict(value, pb.Action()), c)
        self._stop(revoke=True)
        self.config_revision, self.config = self.store.update_config(request.expected_revision, c)
        self.reconnect()
        self.revision += 1
        return self.get_config()

    def require_lease(self, token):
        if self.latched or self.fault or self.applied_revision != self.config_revision:
            fail(grpc.StatusCode.FAILED_PRECONDITION, "设备故障、配置未应用或停止锁定")
        if not self.lease or self.clock() >= self.lease_deadline or not secrets.compare_digest(
                token, self.lease.token):
            fail(grpc.StatusCode.PERMISSION_DENIED, "控制权不存在、已到期或 token 无效")

    def acquire(self, request):
        integer(request.ttl_ms, 200, 5000, "ttl_ms")
        if not request.owner.strip() or len(request.owner) > 128:
            raise ValueError("控制者名称不能为空且不超过 128 字符")
        if self.lease:
            fail(grpc.StatusCode.RESOURCE_EXHAUSTED, f"控制权由 {self.lease.owner} 占用")
        if self.fault or self.applied_revision != self.config_revision:
            fail(grpc.StatusCode.FAILED_PRECONDITION, self.fault or "配置尚未应用")
        if self.latched and not request.clear_latch:
            fail(grpc.StatusCode.FAILED_PRECONDITION, "停止已锁定，请显式 clear_latch")
        try:
            self.driver.begin_control()
        except Exception as e:
            self._device_fault(e)
            fail(grpc.StatusCode.UNAVAILABLE, self.fault)
        self.latched = False
        self.lease = pb.Lease(token=secrets.token_urlsafe(24), owner=request.owner,
                              ttl_ms=request.ttl_ms)
        self.lease_deadline = self.clock() + request.ttl_ms / 1000
        self.revision += 1
        return copy.deepcopy(self.lease)

    def renew(self, request):
        self.require_lease(request.token)
        self.lease_deadline = self.clock() + self.lease.ttl_ms / 1000
        return copy.deepcopy(self.lease)

    def release(self, request):
        self.require_lease(request.token)
        self._stop(revoke=True)
        return pb.Ack(revision=self.revision)

    def _stop(self, revoke=False):
        self.action = None
        self.chassis_target = None
        self.stopped = True
        if revoke:
            self.lease = None
        self.revision += 1
        if self.driver:
            try:
                self.driver.stop()
            except Exception as e:
                self.fault = str(e)
                self.latched = True
                self.applied_revision = 0
                log.error("%s", self.fault)

    def stop(self, request):
        self.latched = self.latched or request.latch
        self._stop(revoke=True)
        return pb.Ack(revision=self.revision)

    def _device_fault(self, error):
        self._stop(revoke=True)
        self.fault = f"设备控制失败：{error}"
        self.latched = True
        self.applied_revision = 0
        self.driver.close()
        log.error("%s", self.fault)

    def _apply(self, targets):
        try:
            self.driver.apply(targets)
            if targets.HasField("chassis"):
                self.chassis_target = copy.deepcopy(targets.chassis)
                self.chassis_deadline = self.clock() + targets.chassis.ttl_ms / 1000
                self.next_chassis = 0
            self.stopped = False
            self.revision += 1
        except Exception as e:
            self._device_fault(e)
            fail(grpc.StatusCode.UNAVAILABLE, self.fault)

    def set_targets(self, request):
        self.require_lease(request.token)
        if self.action:
            fail(grpc.StatusCode.FAILED_PRECONDITION, "动作执行中，先 CancelAction")
        validate_targets(request.targets, self.config)
        self._apply(request.targets)
        return pb.Ack(revision=self.revision)

    def zero_neck(self, request):
        self.require_lease(request.token)
        if not self.config["neck"]["enabled"] or request.mask not in (1, 2, 3):
            raise ValueError("颈部未启用或 mask 无效")
        self._stop()
        try:
            self.driver.zero_neck(request.mask)
        except Exception as e:
            self._device_fault(e)
            fail(grpc.StatusCode.UNAVAILABLE, self.fault)
        self.revision += 1
        return pb.Ack(revision=self.revision)

    def list_actions(self):
        return pb.ActionList(actions=[pb.ActionRecord(revision=rev, deleted=value is None,
                            action=ParseDict(value or {"name": name}, pb.Action()))
                            for name, rev, value in self.store.action_versions()])

    def put_action(self, request):
        validate_action(request.action, self.config)
        if self.action and self.action.name == request.action.name:
            fail(grpc.StatusCode.FAILED_PRECONDITION, "动作正在执行，先取消再修改")
        rev = self.store.put_action(request.action.name, request.expected_revision,
                                   MessageToDict(request.action, preserving_proto_field_name=True))
        return pb.ActionRecord(revision=rev, action=request.action)

    def delete_action(self, request):
        if self.action and self.action.name == request.name:
            fail(grpc.StatusCode.FAILED_PRECONDITION, "动作正在执行，不能删除")
        rev = self.store.delete_action(request.name, request.expected_revision)
        return pb.Ack(revision=rev)

    def run_action(self, request):
        self.require_lease(request.token)
        if self.action:
            fail(grpc.StatusCode.FAILED_PRECONDITION, "已有动作执行中")
        _, value = self.store.action(request.name)
        action = ParseDict(value, pb.Action())
        validate_action(action, self.config)
        self._stop()
        if self.fault:
            fail(grpc.StatusCode.UNAVAILABLE, self.fault)
        self.action = action
        self.action_start = self.clock()
        self.action_index = 0
        return pb.Ack(revision=self.revision)

    def cancel_action(self, request):
        self.require_lease(request.token)
        self._stop()
        return pb.Ack(revision=self.revision)

    def tick(self):
        now = self.clock()
        if self.lease and now >= self.lease_deadline:
            log.info("控制权到期，停止设备：%s", self.lease.owner)
            self._stop(revoke=True)
        if self.fault or not self.driver:
            return
        try:
            if self.action:
                elapsed = (now - self.action_start) * 1000
                if elapsed >= self.action.duration_ms:
                    self._stop()
                elif self.action_index < len(self.action.steps):
                    step = self.action.steps[self.action_index]
                    if elapsed >= step.at_ms:
                        if elapsed - step.at_ms > 250:
                            raise IOError("动作调度延迟超过 250 ms，已停止")
                        self._apply(step.targets)
                        self.action_index += 1
            now = self.clock()
            if self.chassis_target:
                if now >= self.chassis_deadline:
                    self.chassis_target = None
                    self.driver.stop_chassis()
                elif now >= self.next_chassis:
                    self.driver.chassis(self.chassis_target, now)
                    self.next_chassis = now + 0.02
            if self.clock() >= self.next_poll:
                self.driver.poll(self.clock())
                self.next_poll = now + 0.02
        except Exception as e:
            if not self.fault:
                self._device_fault(e)
