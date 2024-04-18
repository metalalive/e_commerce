import random
import unittest
from datetime import date, timedelta
from functools import partial

import jwt
from jwt.api_jwk import PyJWK

from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.auth.jwt      import JwkRsaKeygenHandler
from ecommerce_common.util.python import import_module_string
from ecommerce_common.tests.common import capture_error

from .persistence import _setup_keyfile, _teardown_keyfile


class JwkKeystoreTestCase(unittest.TestCase):
    _init_config = {
        'keystore': 'common.auth.keystore.BaseAuthKeyStore',
        'persist_secret_handler': {
            'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
            'init_kwargs': {
                'filepath': './tmp/cache/test/jwks/privkey/current.json',
                'name':'secret', 'expired_after_days': 7, 'flush_threshold':4,
            },
        },
        'persist_pubkey_handler': {
            'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
            'init_kwargs': {
                'filepath': './tmp/cache/test/jwks/pubkey/current.json',
                'name':'pubkey', 'expired_after_days': 11, 'flush_threshold':4,
            },
        },
    }

    def setUp(self):
        persist_labels = ('persist_pubkey_handler', 'persist_secret_handler')
        self.tear_down_files = {}
        for label in persist_labels:
            filepath = self._init_config[label]['init_kwargs']['filepath']
            del_dir, del_file = _setup_keyfile(filepath=filepath)
            item = {'del_dir':del_dir, 'del_file':del_file, 'filepath':filepath}
            self.tear_down_files[label] = item
        self._init_num_keypairs = 5
        self._keystore = create_keystore_helper(cfg=self._init_config, import_fn=import_module_string)
        self._keygen_handler = JwkRsaKeygenHandler()
        result = self._keystore.rotate(keygen_handler=self._keygen_handler, key_size_in_bits=2048,
                num_keys=self._init_num_keypairs, date_limit=None)
        self._keys_metadata = result['new']


    def tearDown(self):
        for item in self.tear_down_files.values():
            _teardown_keyfile( filepath=item['filepath'], del_dir=item['del_dir'],
                del_file=item['del_file'] )


    def _common_validate_after_rotate(self, filter_key_fn, expect_num_privkeys, expect_num_pubkeys):
        actual_num_privkeys = len(self._keystore._persistence['secret'])
        actual_num_pubkeys  = len(self._keystore._persistence['pubkey'])
        self.assertEqual(expect_num_privkeys, actual_num_privkeys)
        self.assertEqual(expect_num_pubkeys, actual_num_pubkeys)
        for keytype in ('secret', 'pubkey'):
            bound_fn = partial(filter_key_fn, keytype=keytype)
            md_items = filter(bound_fn, self._keys_metadata)
            for md_item in md_items:
                keyitem = self._keystore._persistence[keytype][md_item['kid']]
                self.assertEqual(md_item['alg'], keyitem['alg'])
                self.assertEqual(md_item['exp'], keyitem['exp'])

    def test_rotate_ok(self):
        # subcase 1, add more keys
        for idx in range(1, 4):
            total_num_keys = self._init_num_keypairs + (idx << 1)
            date_rotate = date.today() + timedelta(days=idx)
            result = self._keystore.rotate(keygen_handler=self._keygen_handler, key_size_in_bits=2048,
                    num_keys=total_num_keys, date_limit=date_rotate)
            self.assertFalse(any(result['evict']))
            self._keys_metadata.extend(result['new'])
        filter_key_fn = lambda item, keytype: item['persist_handler'] == keytype
        self._common_validate_after_rotate(filter_key_fn=filter_key_fn, expect_num_privkeys=total_num_keys,\
                expect_num_pubkeys=total_num_keys)
        # subcase 2, assume the date is ahead of expiry time on the first generated private keys
        expired_after_days  = self._init_config['persist_secret_handler']['init_kwargs']['expired_after_days']
        expired_after_days += 1
        date_rotate = date.today() + timedelta(days=expired_after_days)
        result = self._keystore.rotate(keygen_handler=self._keygen_handler, key_size_in_bits=2048,
                num_keys=total_num_keys, date_limit=date_rotate)
        self.assertTrue(any(result['evict']))
        self.assertTrue(any(result['new']))
        evicted_kids = tuple(map(lambda item:item['kid'], result['evict']))
        bound_fn = partial(filter_key_fn, keytype='secret')
        filtered = filter(bound_fn, self._keys_metadata)
        self._keys_metadata = list(filter(lambda item: item['kid'] not in evicted_kids, filtered))
        self._keys_metadata.extend(result['new'])
        expect_num_pubkeys = total_num_keys + self._init_num_keypairs
        self._common_validate_after_rotate(filter_key_fn=filter_key_fn, expect_num_privkeys=total_num_keys,\
                expect_num_pubkeys=expect_num_pubkeys)
        # subcase 3, assume the date is ahead of expiry time on the first generated public keys
        expect_num_privkeys = self._init_num_keypairs + 2
        expired_after_days  = self._init_config['persist_pubkey_handler']['init_kwargs']['expired_after_days']
        expired_after_days += 1
        date_rotate = date.today() + timedelta(days=expired_after_days)
        result = self._keystore.rotate(keygen_handler=self._keygen_handler, key_size_in_bits=2048,
                num_keys=expect_num_privkeys, date_limit=date_rotate)
        self.assertTrue(any(result['evict']))
        self.assertTrue(any(result['new']))
        evicted_kids = tuple(map(lambda item:item['kid'], result['evict']))
        for keytype in ('secret', 'pubkey'):
            bound_fn = partial(filter_key_fn, keytype=keytype)
            filtered = filter(bound_fn, self._keys_metadata)
            self._keys_metadata = list(filter(lambda item: item['kid'] not in evicted_kids, filtered))
        self._keys_metadata.extend(result['new'])
        expect_num_pubkeys += (len(result['new']) >> 1) - self._init_num_keypairs
        self._common_validate_after_rotate(filter_key_fn=filter_key_fn, expect_num_privkeys=expect_num_privkeys,\
                expect_num_pubkeys=expect_num_pubkeys)
    ## end of test_rotate_ok()


    def test_rotate_no_change(self):
        expired_after_days  = self._init_config['persist_secret_handler']['init_kwargs']['expired_after_days']
        expired_after_days -= 1
        date_rotate = date.today() + timedelta(days=expired_after_days)
        result = self._keystore.rotate(keygen_handler=self._keygen_handler, key_size_in_bits=2048,
                num_keys=2, date_limit=None)
        self.assertFalse(any(result['evict']))
        item = result['new'][0]
        self.assertLessEqual(item['next_num_keys'], item['curr_num_keys'])
        self.assertEqual(item['msg'], 'no new key generated')

    def test_rand_choose_keys(self):
        with self.assertRaises(AssertionError):
            self._keystore.choose_secret(kid=None, randonly=False)
        for _ in range(50):
            rawdata_privkey = self._keystore.choose_secret(randonly=True)
            rawdata_pubkey  = self._keystore.choose_pubkey(kid=rawdata_privkey['kid'])
            self._validate_chosen_keypair(rawdata_privkey, rawdata_pubkey)

    def test_choose_specific_keys(self):
        avail_kids_gen = map(lambda item:item['kid'], self._keys_metadata)
        for key_id in avail_kids_gen:
            rawdata_privkey = self._keystore.choose_secret(kid=key_id)
            rawdata_pubkey  = self._keystore.choose_pubkey(kid=key_id)
            self._validate_chosen_keypair(rawdata_privkey, rawdata_pubkey)

    def _validate_chosen_keypair(self, rawdata_privkey, rawdata_pubkey):
        jwk_priv = PyJWK(jwk_data=rawdata_privkey)
        jwk_pub  = PyJWK(jwk_data=rawdata_pubkey)
        expect_payld = {'some':'payload', 'avoid':'sensitive', 'info':'leak'}
        encoded_token = jwt.encode(expect_payld, jwk_priv.key, algorithm=rawdata_privkey['alg'], headers={})
        decoded_payld = jwt.decode(encoded_token, jwk_pub.key, algorithms=rawdata_pubkey['alg'])
        self.assertDictEqual(expect_payld, decoded_payld)


