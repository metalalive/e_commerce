import json
import http.server
import socketserver
import argparse
import os
import ssl
from typing import Dict, Tuple

from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives import serialization
from jwt.utils import base64url_encode

def gen_rsa_keys(num: int, keysize: int) -> Tuple[Dict, Dict]:
    """Generate a specified number of RSA keys and return them as public and private JWKS objects."""
    public_keys = []
    private_keys = []

    for i in range(num):
        private_key = rsa.generate_private_key(
            public_exponent=65537,
            key_size=keysize
        )
        public_key = private_key.public_key()

        # Serialize public key
        public_jwk = {
            "kty": "RSA",
            "kid": f"key-{i+1}",
            "use": "sig",
            "alg": "RS256",
            "n": base64url_encode(public_key.public_numbers().n.to_bytes((public_key.public_numbers().n.bit_length() + 7) // 8, "big")).decode("utf-8"),
            "e": base64url_encode(public_key.public_numbers().e.to_bytes((public_key.public_numbers().e.bit_length() + 7) // 8, "big")).decode("utf-8")
        }
        public_keys.append(public_jwk)

        priv_nums = private_key.private_numbers()
        private_jwk = {
            "kty": "RSA",
            "kid": f"key-{i+1}",
            "use": "sig",
            "alg": "RS256",
            "n": public_jwk['n'],
            "e": public_jwk['e'],
            "d": base64url_encode(priv_nums.d.to_bytes((priv_nums.d.bit_length() + 7) // 8, "big")).decode("utf-8"),
            "p": base64url_encode(priv_nums.p.to_bytes((priv_nums.p.bit_length() + 7) // 8, "big")).decode("utf-8"),
            "q": base64url_encode(priv_nums.q.to_bytes((priv_nums.q.bit_length() + 7) // 8, "big")).decode("utf-8"),
            "dp": base64url_encode(priv_nums.dmp1.to_bytes((priv_nums.dmp1.bit_length() + 7) // 8, "big")).decode("utf-8"),
            "dq": base64url_encode(priv_nums.dmq1.to_bytes((priv_nums.dmq1.bit_length() + 7) // 8, "big")).decode("utf-8"),
            "qi": base64url_encode(priv_nums.iqmp.to_bytes((priv_nums.iqmp.bit_length() + 7) // 8, "big")).decode("utf-8")
        }
        private_keys.append(private_jwk)

    return {"keys": public_keys}, {"keys": private_keys}

class JWKSHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/jwks":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(serial_pubkeys).encode("utf-8"))
        else:
            self.send_response(404)
            self.end_headers()

# Parse arguments for HOST, PORT, and private key file path
def parse_args():
    parser = argparse.ArgumentParser(description="Run a mock JWKS server.")
    parser.add_argument("--host", type=str, required=True, default="localhost")
    parser.add_argument("--port", type=int, required=True, default=8008)
    parser.add_argument("--path2privkey", type=str, required=True)
    parser.add_argument("--sslcertfile", type=str, required=True, help="Path to the server certificate.")
    parser.add_argument("--sslkeyfile", type=str, required=True, help="Path to the server private key.")
    return parser.parse_args()

if __name__ == "__main__":
    args = parse_args()
    HOST = args.host
    PORT = args.port
    PATH_TO_PRIVKEY = args.path2privkey
    CERTFILE = args.sslcertfile
    KEYFILE = args.sslkeyfile

    serial_pubkeys, serial_privkeys = gen_rsa_keys(num=3, keysize=2048)

    # Save private keys to the specified file
    with open(PATH_TO_PRIVKEY, "w") as priv_file:
        json.dump(serial_privkeys, priv_file, indent=4)

    # Run the HTTP server
    with socketserver.TCPServer((HOST, PORT), JWKSHandler) as httpd:
        httpd.socket = ssl.wrap_socket(
            httpd.socket,
            server_side=True,
            certfile=CERTFILE,
            keyfile=KEYFILE,
            ssl_version=ssl.PROTOCOL_TLS
        )
        print(f"Serving JWKS on https://{HOST}:{PORT}...")
        httpd.serve_forever()

