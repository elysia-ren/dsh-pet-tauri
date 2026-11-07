"""dsh-pet 本地 mock：OpenAI 兼容 chat + DeepSeek 余额。仅用于本地联调。"""
import json
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = 3199


class Handler(BaseHTTPRequestHandler):
    def _json(self, obj, code=200):
        body = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/user/balance":
            self._json({
                "is_available": True,
                "balance_infos": [{
                    "currency": "CNY",
                    "total_balance": "12.50",
                    "granted_balance": "2.50",
                    "topped_up_balance": "10.00",
                }],
            })
        else:
            self._json({"error": {"message": "not found"}}, 404)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        try:
            req = json.loads(raw)
        except Exception:
            req = {}
        if self.path.endswith("/chat/completions"):
            time.sleep(1.0)  # 留出「思考动画」的观察窗口
            msgs = req.get("messages") or []
            last = ""
            wants_image = False
            for m in msgs:
                if m.get("role") == "user":
                    last = m.get("content", "")
                    if "[配图]" in last:
                        wants_image = True
            reply = "喵～收到：" + last[:30] if last else "喵～"
            if wants_image:
                reply += "\n[图:可爱]"
            self._json({
                "id": "mock",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": reply}}],
            })
        else:
            self._json({"error": {"message": "not found"}}, 404)

    def log_message(self, fmt, *args):
        print("[mock]", fmt % args, flush=True)


if __name__ == "__main__":
    print(f"[mock] http://127.0.0.1:{PORT}", flush=True)
    HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
