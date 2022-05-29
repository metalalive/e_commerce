
"""
This script currently renews SSL certificates for testing and development purpose
"""
from datetime import datetime, timedelta
from functools import partial
import sys
import json
import argparse
import pdb

from cryptography import x509
from cryptography.x509.oid import  NameOID
from cryptography.x509.extensions import  SubjectAlternativeName
from cryptography.hazmat.primitives import _serialization, hashes
from cryptography.hazmat.primitives.asymmetric import rsa

from common.util.python import import_module_string

def check_cert_expiry(listen):
    item = {}
    cert_file = None
    ssl_setup = listen['ssl']
    try:
        cert_file = open(ssl_setup['cert_file'], 'rb')
        cert = x509.load_pem_x509_certificate(cert_file.read())
        if cert.not_valid_after < datetime.utcnow():
            item = {'cert':ssl_setup['cert_file']}
    except ValueError as e:
        can_handle = e.args[0].startswith('Unable to load PEM file')
        if can_handle:
            item = {'cert':ssl_setup['cert_file']}
        raise
    except (FileNotFoundError,) as e:
        item = {'cert':ssl_setup['cert_file']}
    finally:
        if cert_file and not cert_file.closed:
            cert_file.close()
    if item.get('cert') :
        item['privkey'] = ssl_setup['privkey_file']
        item['host'] = listen['host']
    return item


class DevCertRenewal:
    def start(self, argv:list):
        assert len(argv) == 1, "arguments must include (1) app config file"
        setting_path  = argv[0]
        f = None
        renew_required = []
        cfg_root = {}
        try:
            f = open(setting_path, 'r')
            cfg_root = json.load(f)
            renew_required = map(check_cert_expiry, cfg_root['listen'])
            renew_required = list(filter(any, renew_required))
        finally:
            f.close()
        self.run_renewal(renew_required, cfg_root)

    # TODO, this function is used only for testing / development purpose
    # , for production it should be `certbot` that handles the renewal
    def run_renewal(self, renew_required, cfg_root):
        if not renew_required:
            print('Server certificates still valid, nothing to renew')
            return # all certs are still valid , no need to renew
        assert cfg_root.get('ca') , 'missing object field `ca` in json config file'
        ca_privkey = self.create_test_privkey(wr_pem_path=cfg_root['ca']['privkey_file'], key_sz=4096)
        ca_cert = self.create_test_ca(wr_pem_path=cfg_root['ca']['cert_file'], privkey=ca_privkey)
        for item in renew_required:
            self.run_renewal_item(req=item, ca_privkey=ca_privkey, ca_cert=ca_cert)
        print('renew certificates successfully')

    def create_test_ca(self, wr_pem_path:str, privkey):
        assert privkey,    'privkey must NOT be null'
        issuer_name  = x509.Name([
            x509.NameAttribute(oid=NameOID.COUNTRY_NAME, value='TW' ),
            x509.NameAttribute(oid=NameOID.ORGANIZATION_NAME, value='CA organization' ),
            x509.NameAttribute(oid=NameOID.COMMON_NAME,  value='app_tester_ca' )
        ])
        builder = self.create_test_cert_builder(pubkey=privkey.public_key(), issuer_name=issuer_name,
                subject_name=issuer_name, key_cert_sign=True, crl_sign=True)
        basic_constraint = x509.BasicConstraints(ca=True, path_length=0)
        builder = builder.add_extension(extval=basic_constraint, critical=True)
        cert = builder.sign(private_key=privkey, algorithm=hashes.SHA384())
        pem_data_cert = cert.public_bytes(encoding=_serialization.Encoding.PEM)
        with open(wr_pem_path, 'wb') as f:
            f.write(pem_data_cert)
        return cert

    def create_test_server_cert(self, wr_pem_path:str, privkey, ca_privkey, ca_cert, subj_alt_name:str):
        assert subj_alt_name, 'subj_alt_name must not be null, the value: %s' % subj_alt_name
        assert privkey,    'privkey must NOT be null'
        assert ca_privkey, 'ca_privkey must NOT be null'
        assert ca_cert,    'ca_cert must NOT be null'
        subj_name  = x509.Name([
            x509.NameAttribute(oid=NameOID.COUNTRY_NAME, value='SG' ),
            x509.NameAttribute(oid=NameOID.ORGANIZATION_NAME, value='Service Provider' ),
            x509.NameAttribute(oid=NameOID.COMMON_NAME,  value='app_tester' )
        ])
        builder = self.create_test_cert_builder(pubkey=privkey.public_key(), issuer_name=ca_cert.issuer,
                subject_name=subj_name)
        basic_constraint = x509.BasicConstraints(ca=False, path_length=None)
        builder = builder.add_extension(extval=basic_constraint, critical=True)
        dns_names = [x509.DNSName(value=subj_alt_name)]
        san = SubjectAlternativeName(general_names=dns_names)
        builder = builder.add_extension(extval=san, critical=True)
        cert = builder.sign(private_key=ca_privkey, algorithm=hashes.SHA256())
        pem_data_cert = cert.public_bytes(encoding=_serialization.Encoding.PEM)
        with open(wr_pem_path, 'wb') as f:
            f.write(pem_data_cert)
        return cert

    def run_renewal_item(self, req, ca_privkey, ca_cert):
        srv_privkey = self.create_test_privkey(wr_pem_path=req['privkey'])
        srv_cert = self.create_test_server_cert(wr_pem_path=req['cert'], privkey=srv_privkey,
                ca_cert=ca_cert, ca_privkey=ca_privkey, subj_alt_name=req['host'])


    def create_test_privkey(self, wr_pem_path:str, key_sz:int = 2048, pub_e:int = 0x10001,
            encrypt_privkey_passwd:bytes = b'' ):
        privkey = rsa.generate_private_key(public_exponent=pub_e, key_size=key_sz)
        if any(encrypt_privkey_passwd):
            encryption_algorithm = _serialization.BestAvailableEncryption(encrypt_privkey_passwd)
        else:
            encryption_algorithm = _serialization.NoEncryption()
        pem_data_privkey = privkey.private_bytes(
            encoding=_serialization.Encoding.PEM ,
            format=_serialization.PrivateFormat.PKCS8 ,
            encryption_algorithm=encryption_algorithm,
        )
        with open(wr_pem_path, 'wb') as f:
            f.write(pem_data_privkey)
        return privkey


    def create_test_cert_builder(self,  pubkey, issuer_name:x509.Name, subject_name:x509.Name, days_expiry:int = 5,
            not_valid_before:datetime = None,  not_valid_after:datetime = None,
            digital_signature:bool=True, key_encipherment:bool=False, data_encipherment:bool=False,
            key_agreement:bool=False, key_cert_sign:bool=False, crl_sign:bool=False ):
        if not not_valid_before:
            not_valid_before = datetime.utcnow()
        if not not_valid_after:
            not_valid_after = not_valid_before + timedelta(days=days_expiry)
        builder = x509.CertificateBuilder(
                public_key = pubkey,
                issuer_name  = issuer_name ,
                subject_name = subject_name,
                not_valid_before = not_valid_before,
                not_valid_after  = not_valid_after
            )
        builder = builder.serial_number(x509.random_serial_number())
        key_use = x509.KeyUsage(
                digital_signature  = digital_signature ,
                key_encipherment   = key_encipherment  ,
                data_encipherment  = data_encipherment ,
                key_agreement      = key_agreement     ,
                key_cert_sign      = key_cert_sign     ,
                crl_sign           = crl_sign          ,
                content_commitment = False,
                encipher_only  = False,
                decipher_only  = False,
            )
        builder = builder.add_extension(extval=key_use, critical=True)
        return builder
## end of class DevCertRenewal

class TestCertRenewal(DevCertRenewal):
    pass


__all__ = ['TestCertRenewal', 'DevCertRenewal']

