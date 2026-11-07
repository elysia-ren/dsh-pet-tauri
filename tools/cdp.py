"""极简 CDP 客户端：在 Tauri WebView2 页面里执行 JS，用于本地联调。

用法:
  python tools/cdp.py list
  python tools/cdp.py eval <url 子串> "<js 表达式>"

应用需带 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223 启动。
"""
import json
import sys
import urllib.request

import websocket


def pages():
    with urllib.request.urlopen("http://127.0.0.1:9223/json", timeout=5) as resp:
        return json.loads(resp.read().decode("utf-8"))


def find_page(needle):
    for page in pages():
        if needle in (page.get("url") or "") or needle in (page.get("title") or ""):
            return page
    raise SystemExit(f"page not found: {needle}")


def eval_js(page, expression):
    ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=15)
    try:
        ws.send(json.dumps({
            "id": 1,
            "method": "Runtime.evaluate",
            "params": {
                "expression": expression,
                "awaitPromise": True,
                "returnByValue": True,
            },
        }))
        while True:
            msg = json.loads(ws.recv())
            if msg.get("id") != 1:
                continue
            result = msg.get("result", {})
            if "exceptionDetails" in result:
                print(json.dumps(result["exceptionDetails"], ensure_ascii=False, indent=2))
                return 1
            value = result.get("result", {}).get("value")
            if isinstance(value, (dict, list)):
                print(json.dumps(value, ensure_ascii=False, indent=2))
            else:
                print(value)
            return 0
    finally:
        ws.close()


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    cmd = sys.argv[1]
    if cmd == "list":
        for page in pages():
            print(f'{page.get("title")!r}\t{page.get("url")}\t{page.get("id")}')
        return 0
    if cmd == "eval":
        page = find_page(sys.argv[2])
        return eval_js(page, sys.argv[3])
    raise SystemExit(__doc__)


if __name__ == "__main__":
    sys.exit(main())
