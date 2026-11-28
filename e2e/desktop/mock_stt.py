"""Groq-compatible transcription mock for desktop E2E and the latency harness.
Returns a fixed transcript and records the size of each uploaded WAV (never its
content). MOCK_STT_DELAY_MS simulates provider latency."""
import http.server, json, os, sys, time

DELAY = int(os.environ.get("MOCK_STT_DELAY_MS", "0")) / 1000
TRANSCRIPT = sys.argv[2] if len(sys.argv) > 2 else "hello from the end to end test"
LOG = sys.argv[3] if len(sys.argv) > 3 else None

class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get("content-length", 0))
        body = self.rfile.read(n)
        if DELAY:
            time.sleep(DELAY)
        if LOG:
            with open(LOG, "a") as f:
                f.write(json.dumps({"path": self.path, "bytes": len(body), "auth": self.headers.get("authorization", "")[:7]}) + "\n")
        out = json.dumps({"text": TRANSCRIPT}).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass

http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
