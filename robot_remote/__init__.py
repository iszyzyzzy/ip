"""通过 HTTP/WebSocket 接入 robotd 的独立遥控网关。"""

HTTP_STATUS = {
    "INVALID_ARGUMENT": 400, "UNAUTHENTICATED": 401, "PERMISSION_DENIED": 403,
    "NOT_FOUND": 404, "ABORTED": 409, "FAILED_PRECONDITION": 409,
    "RESOURCE_EXHAUSTED": 429, "UNAVAILABLE": 503, "DEADLINE_EXCEEDED": 504,
}


class RemoteError(Exception):
    def __init__(self, code, message, status=None):
        super().__init__(message)
        self.code = code
        self.status = status or HTTP_STATUS.get(code, 500)

    def payload(self):
        return {"code": self.code, "message": str(self)}
