import json
import asyncio
import argparse
import os
import ssl
from pathlib import Path
from typing import Dict, Tuple

from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives import serialization
from jwt.utils import base64url_encode

from infra.renew_certs import check_cert_expiry, DevCertRenewal


def gen_rsa_keys(num: int, keysize: int) -> Tuple[Dict, Dict]:
    """Generate a specified number of RSA keys and return them as public and private JWKS objects."""
    public_keys = []
    private_keys = []

    for i in range(num):
        private_key = rsa.generate_private_key(public_exponent=65537, key_size=keysize)
        public_key = private_key.public_key()

        # Serialize public key
        public_jwk = {
            "kty": "RSA",
            "kid": f"key-{i+1}",
            "use": "sig",
            "alg": "RS256",
            "n": base64url_encode(
                public_key.public_numbers().n.to_bytes(
                    (public_key.public_numbers().n.bit_length() + 7) // 8, "big"
                )
            ).decode("utf-8"),
            "e": base64url_encode(
                public_key.public_numbers().e.to_bytes(
                    (public_key.public_numbers().e.bit_length() + 7) // 8, "big"
                )
            ).decode("utf-8"),
        }
        public_keys.append(public_jwk)

        priv_nums = private_key.private_numbers()
        private_jwk = {
            "kty": "RSA",
            "kid": f"key-{i+1}",
            "use": "sig",
            "alg": "RS256",
            "n": public_jwk["n"],
            "e": public_jwk["e"],
            "d": base64url_encode(
                priv_nums.d.to_bytes((priv_nums.d.bit_length() + 7) // 8, "big")
            ).decode("utf-8"),
            "p": base64url_encode(
                priv_nums.p.to_bytes((priv_nums.p.bit_length() + 7) // 8, "big")
            ).decode("utf-8"),
            "q": base64url_encode(
                priv_nums.q.to_bytes((priv_nums.q.bit_length() + 7) // 8, "big")
            ).decode("utf-8"),
            "dp": base64url_encode(
                priv_nums.dmp1.to_bytes((priv_nums.dmp1.bit_length() + 7) // 8, "big")
            ).decode("utf-8"),
            "dq": base64url_encode(
                priv_nums.dmq1.to_bytes((priv_nums.dmq1.bit_length() + 7) // 8, "big")
            ).decode("utf-8"),
            "qi": base64url_encode(
                priv_nums.iqmp.to_bytes((priv_nums.iqmp.bit_length() + 7) // 8, "big")
            ).decode("utf-8"),
        }
        private_keys.append(private_jwk)

    return {"keys": public_keys}, {"keys": private_keys}


async def mock_app(scope, receive, send):
    if scope["type"] == "http" and scope["path"] == "/jwks":
        await send(
            {
                "type": "http.response.start",
                "status": 200,
                "headers": [[b"content-type", b"application/json"]],
            }
        )
        await send(
            {"type": "http.response.body", "body": json.dumps(serial_pubkeys).encode("utf-8")}
        )
    else:
        await send({"type": "http.response.start", "status": 404, "headers": []})
        await send({"type": "http.response.body", "body": b""})


# Parse arguments for HOST, PORT, and private key file path
def parse_args():
    parser = argparse.ArgumentParser(description="Run a mock JWKS server.")
    parser.add_argument("--host", type=str, required=True, default="localhost")
    parser.add_argument("--port", type=int, required=True, default=8008)
    parser.add_argument(
        "--path2privkey", type=str, required=True, help="Path to private keys for JWK."
    )
    parser.add_argument(
        "--sslcertpath", type=str, required=True, help="Path to the server certificate."
    )
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_args()
    HOST = args.host
    PORT = args.port
    PATH_JWKS_PRIVKEY = args.path2privkey
    cert_path = Path(args.sslcertpath).resolve(strict=True)
    ca_cfg = {
        "cert_file": cert_path.joinpath("ca.crt"),
        "privkey_file": cert_path.joinpath("ca.private.key"),
    }
    server_cert_cfg = {
        "host": HOST,
        "port": PORT,
        "ssl": {
            "cert_file": cert_path.joinpath("%s_%d.crt" % (HOST, PORT)),
            "privkey_file": cert_path.joinpath("%s_%d.private.key" % (HOST, PORT)),
        },
    }
    renewal = DevCertRenewal()
    renew_ca = renewal.check_ca_renew(ca_cfg)
    renew_auth_server = check_cert_expiry(listen=server_cert_cfg)
    renewal.run_renewal([renew_auth_server], renew_ca)

    # ------ key pairs for JWKS ------
    serial_pubkeys, serial_privkeys = gen_rsa_keys(num=3, keysize=2048)

    with open(PATH_JWKS_PRIVKEY, "w") as priv_file:
        json.dump(serial_privkeys, priv_file, indent=4)

    from hypercorn.asyncio import serve
    from hypercorn.config import Config

    config = Config()
    config.bind = [f"{HOST}:{PORT}"]
    config.certfile = str(server_cert_cfg["ssl"]["cert_file"])
    config.keyfile = str(server_cert_cfg["ssl"]["privkey_file"])
    config.alpn_protocols = ["h2", "http/1.1"]  # Enable HTTP/2
    config.accesslog = "-"
    print(f"Serving JWKS on https://{HOST}:{PORT}...")
    asyncio.run(serve(mock_app, config))
