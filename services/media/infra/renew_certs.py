"""
This script currently renews SSL certificates for testing and development purpose
"""

from datetime import datetime, timedelta
from functools import partial
import sys
import json
import argparse
import os

from cryptography import x509
from cryptography.x509.oid import NameOID
from cryptography.x509.extensions import SubjectAlternativeName
from cryptography.hazmat.backends import default_backend as crypto_default_backend
from cryptography.hazmat.primitives import _serialization, hashes
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.serialization import load_pem_private_key


def check_cert_expiry(listen):
    item = {}
    cert_file = None
    ssl_setup = listen["ssl"]
    cert_filepath = ssl_setup["cert_file"]
    pkey_filepath = ssl_setup["privkey_file"]
    try:
        cert_file = open(cert_filepath, "rb")
        cert = x509.load_pem_x509_certificate(cert_file.read())
        if cert.not_valid_after < datetime.utcnow():
            item = {"cert": cert_filepath}
    except ValueError as e:
        can_handle = e.args[0].startswith("Unable to load PEM file")
        if can_handle:
            item = {"cert": cert_filepath}
        raise
    except (FileNotFoundError,) as e:
        item = {"cert": cert_filepath}
    finally:
        if cert_file and not cert_file.closed:
            cert_file.close()
    if item.get("cert"):
        item["privkey"] = pkey_filepath
        item["host"] = listen.get("host")
    return item


class DevCertRenewal:
    def start(self, argv: list):
        raise NotImplementedError()

    def check_ca_renew(self, cfg: dict):
        out = check_cert_expiry({"ssl": cfg})
        out["renew"] = any(out)
        if out["renew"] is False:
            out["cert"] = cfg["cert_file"]
            out["privkey"] = cfg["privkey_file"]
        return out

    # TODO, this function is used only for testing / development purpose
    # , for production it should be `certbot` that handles the renewal
    def run_renewal(self, renew_servers, renew_ca):
        ca_privkey = None
        ca_cert = None
        num_renew_done = 0
        if renew_ca["renew"] is True:
            ca_privkey = self.create_test_privkey(wr_pem_path=renew_ca["privkey"], key_sz=4096)
            ca_cert = self.create_test_ca(wr_pem_path=renew_ca["cert"], privkey=ca_privkey)
        else:  # load ca and its pkey
            with open(renew_ca["privkey"], "rb") as f:
                ca_privkey = load_pem_private_key(f.read(), None, crypto_default_backend())
            assert ca_privkey and isinstance(
                ca_privkey, rsa.RSAPrivateKey
            ), "loaded invalid private key for CA cert"
            with open(renew_ca["cert"], "rb") as f:
                ca_cert = x509.load_pem_x509_certificate(f.read())
            assert ca_cert, "loaded invalid CA cert"
        for item in renew_servers:
            if any(item):
                self.run_renewal_item(req=item, ca_privkey=ca_privkey, ca_cert=ca_cert)
                num_renew_done += 1
        if num_renew_done > 0:
            print("renew certificates successfully")
        else:
            print("Server certificates still valid, nothing to renew")

    def create_test_ca(self, wr_pem_path: str, privkey):
        assert privkey, "privkey must NOT be null"
        issuer_name = x509.Name(
            [
                x509.NameAttribute(oid=NameOID.COUNTRY_NAME, value="TW"),
                x509.NameAttribute(oid=NameOID.ORGANIZATION_NAME, value="CA organization"),
                x509.NameAttribute(oid=NameOID.COMMON_NAME, value="app_tester_ca"),
            ]
        )
        builder = self.create_test_cert_builder(
            pubkey=privkey.public_key(),
            issuer_name=issuer_name,
            subject_name=issuer_name,
            key_cert_sign=True,
            crl_sign=True,
        )
        basic_constraint = x509.BasicConstraints(ca=True, path_length=0)
        builder = builder.add_extension(extval=basic_constraint, critical=True)
        cert = builder.sign(private_key=privkey, algorithm=hashes.SHA384())
        pem_data_cert = cert.public_bytes(encoding=_serialization.Encoding.PEM)
        with open(wr_pem_path, "wb") as f:
            f.write(pem_data_cert)
        return cert

    def create_test_server_cert(
        self, wr_pem_path: str, privkey, ca_privkey, ca_cert, subj_alt_name: str
    ):
        assert subj_alt_name, "subj_alt_name must not be null, the value: %s" % subj_alt_name
        assert privkey, "privkey must NOT be null"
        assert ca_privkey, "ca_privkey must NOT be null"
        assert ca_cert, "ca_cert must NOT be null"
        subj_name = x509.Name(
            [
                x509.NameAttribute(oid=NameOID.COUNTRY_NAME, value="SG"),
                x509.NameAttribute(oid=NameOID.ORGANIZATION_NAME, value="Service Provider"),
                x509.NameAttribute(oid=NameOID.COMMON_NAME, value="app_tester"),
            ]
        )
        builder = self.create_test_cert_builder(
            pubkey=privkey.public_key(), issuer_name=ca_cert.issuer, subject_name=subj_name
        )
        basic_constraint = x509.BasicConstraints(ca=False, path_length=None)
        builder = builder.add_extension(extval=basic_constraint, critical=True)
        dns_names = [x509.DNSName(value=subj_alt_name)]
        san = SubjectAlternativeName(general_names=dns_names)
        builder = builder.add_extension(extval=san, critical=True)
        cert = builder.sign(private_key=ca_privkey, algorithm=hashes.SHA256())
        pem_data_cert = cert.public_bytes(encoding=_serialization.Encoding.PEM)
        with open(wr_pem_path, "wb") as f:
            f.write(pem_data_cert)
        return cert

    def run_renewal_item(self, req, ca_privkey, ca_cert):
        srv_privkey = self.create_test_privkey(wr_pem_path=req["privkey"])
        srv_cert = self.create_test_server_cert(
            wr_pem_path=req["cert"],
            privkey=srv_privkey,
            ca_cert=ca_cert,
            ca_privkey=ca_privkey,
            subj_alt_name=req["host"],
        )

    def create_test_privkey(
        self,
        wr_pem_path: str,
        key_sz: int = 2048,
        pub_e: int = 0x10001,
        encrypt_privkey_passwd: bytes = b"",
    ):
        privkey = rsa.generate_private_key(public_exponent=pub_e, key_size=key_sz)
        if any(encrypt_privkey_passwd):
            encryption_algorithm = _serialization.BestAvailableEncryption(encrypt_privkey_passwd)
        else:
            encryption_algorithm = _serialization.NoEncryption()
        pem_data_privkey = privkey.private_bytes(
            encoding=_serialization.Encoding.PEM,
            format=_serialization.PrivateFormat.PKCS8,
            encryption_algorithm=encryption_algorithm,
        )
        with open(wr_pem_path, "wb") as f:
            f.write(pem_data_privkey)
        return privkey

    def create_test_cert_builder(
        self,
        pubkey,
        issuer_name: x509.Name,
        subject_name: x509.Name,
        days_expiry: int = 5,
        not_valid_before: datetime = None,
        not_valid_after: datetime = None,
        digital_signature: bool = True,
        key_encipherment: bool = False,
        data_encipherment: bool = False,
        key_agreement: bool = False,
        key_cert_sign: bool = False,
        crl_sign: bool = False,
    ):
        if not not_valid_before:
            not_valid_before = datetime.utcnow()
        if not not_valid_after:
            not_valid_after = not_valid_before + timedelta(days=days_expiry)
        builder = x509.CertificateBuilder(
            public_key=pubkey,
            issuer_name=issuer_name,
            subject_name=subject_name,
            not_valid_before=not_valid_before,
            not_valid_after=not_valid_after,
        )
        builder = builder.serial_number(x509.random_serial_number())
        key_use = x509.KeyUsage(
            digital_signature=digital_signature,
            key_encipherment=key_encipherment,
            data_encipherment=data_encipherment,
            key_agreement=key_agreement,
            key_cert_sign=key_cert_sign,
            crl_sign=crl_sign,
            content_commitment=False,
            encipher_only=False,
            decipher_only=False,
        )
        builder = builder.add_extension(extval=key_use, critical=True)
        return builder

    def _process_renewal_flow(self, argv: list, server_certs_cfg_getter):
        assert len(argv) == 1, "arguments must include (1) app config file"
        sys_basepath = os.getenv("SYS_BASE_PATH")
        setting_path = argv[0]
        cfg_fullpath = f"{sys_basepath}/{setting_path}"
        with open(cfg_fullpath, "r") as f:
            cfg_root = json.load(f)
            ca_cfg = cfg_root["ca"]
            ca_cfg["cert_file"] = os.path.join(sys_basepath, ca_cfg["cert_file"])
            ca_cfg["privkey_file"] = os.path.join(sys_basepath, ca_cfg["privkey_file"])
            renew_ca = self.check_ca_renew(ca_cfg)

            server_configs = server_certs_cfg_getter(cfg_root)
            for srv_cfg_item in server_configs:
                if "ssl" in srv_cfg_item:
                    srv_cfg_item["ssl"]["cert_file"] = os.path.join(
                        sys_basepath, srv_cfg_item["ssl"]["cert_file"]
                    )
                    srv_cfg_item["ssl"]["privkey_file"] = os.path.join(
                        sys_basepath, srv_cfg_item["ssl"]["privkey_file"]
                    )

            renew_servers = map(check_cert_expiry, server_configs)
            self.run_renewal(renew_servers, renew_ca)


class AppSrvCertRenewal(DevCertRenewal):
    def start(self, argv: list):
        self._process_renewal_flow(argv, lambda cfg: cfg["listen"])


class AppCdnCertRenewal(DevCertRenewal):
    def start(self, argv: list):
        self._process_renewal_flow(argv, lambda cfg: [cfg["proxy"]])


__all__ = ["AppSrvCertRenewal", "AppCdnCertRenewal"]
