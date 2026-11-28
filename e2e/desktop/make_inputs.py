"""Writes a 16 kHz mono speech-like WAV and an HS256 test JWT (audience 'authenticated')."""
import base64, hashlib, hmac, json, math, struct, sys, time, wave

wav_path, secret = sys.argv[1], sys.argv[2]
rate, secs = 16000, 2.0
with wave.open(wav_path, "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(rate)
    frames = bytearray()
    for i in range(int(rate * secs)):
        env = 0.5 + 0.5 * math.sin(2 * math.pi * 3 * i / rate)  # syllable-like amplitude
        s = int(12000 * env * math.sin(2 * math.pi * 220 * i / rate))
        frames += struct.pack("<h", s)
    w.writeframes(bytes(frames))

def b64(d):
    return base64.urlsafe_b64encode(d).rstrip(b"=").decode()

header = b64(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
payload = b64(json.dumps({"sub": "e2e-user", "aud": "authenticated", "exp": int(time.time()) + 3600}).encode())
sig = b64(hmac.new(secret.encode(), f"{header}.{payload}".encode(), hashlib.sha256).digest())
print(f"{header}.{payload}.{sig}")
