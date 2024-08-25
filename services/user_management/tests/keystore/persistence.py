import os
import random
import shutil
import unittest
from datetime import date, timedelta
from pathlib import Path

from ecommerce_common.auth.keystore import JWKSFilePersistHandler
from ecommerce_common.tests.common import (
    capture_error,
    _setup_keyfile,
    _teardown_keyfile,
)


def _clean_prev_persisted_filedata(**init_kwargs):
    today = date.today()
    init_kwargs.update({"flush_threshold": 9999, "auto_flush": True})
    persist_handler = JWKSFilePersistHandler(**init_kwargs)
    persist_handler.evict_expired_keys(
        date_limit=today + timedelta(days=persist_handler.max_expired_after_days)
    )
    persist_handler.flush()
    assert len(persist_handler) == 0


srv_basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)


class FilePersistHandlerTestCase(unittest.TestCase):
    _init_kwargs = {
        "filepath": os.path.join(
            srv_basepath, "./tmp/cache/test/jwks/privkey/current.json"
        ),
        "name": "test_secret_storage",
        "expired_after_days": 11,
        "max_expired_after_days": 90,
        "flush_threshold": 4,
        "auto_flush": False,
    }

    def setUp(self):
        dir_tear_down, file_tear_down = _setup_keyfile(
            filepath=self._init_kwargs["filepath"]
        )
        self._dir_tear_down = dir_tear_down
        self._file_tear_down = file_tear_down
        _clean_prev_persisted_filedata(**self._init_kwargs.copy())

    def tearDown(self):
        _teardown_keyfile(
            filepath=self._init_kwargs["filepath"],
            del_dir=self._dir_tear_down,
            del_file=self._file_tear_down,
        )

    def test_save_key_items(self):
        persist_handler = JWKSFilePersistHandler(**self._init_kwargs)
        max_expired_after_days = persist_handler.max_expired_after_days
        today = date.today()
        # -------- subcase #1, add 4 items and manually flush
        keydata = {
            "crypto-key-id-%s"
            % idx: {
                "exp": (
                    today + timedelta(days=random.randrange(2, max_expired_after_days))
                ).isoformat(),
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
                "use": "Signature",
            }
            for idx in range(4)
        }
        self.assertEqual(
            persist_handler.expired_after_days.days,
            self._init_kwargs["expired_after_days"],
        )
        for key, item in keydata.items():
            self.assertFalse(persist_handler.is_full)
            persist_handler[key] = item
        self.assertTrue(persist_handler.is_full)
        self.assertEqual(0, len(persist_handler))
        persist_handler.flush()  # manual flush
        self.assertFalse(persist_handler.is_full)
        self.assertEqual(len(keydata.keys()), len(persist_handler))
        actual_data = dict(persist_handler.items())
        expect_data = {k: {} for k in keydata.keys()}
        self.assertDictEqual(actual_data, expect_data)
        actual_data_gen = persist_handler.items(
            present_fields=["exp", "kty", "use", "alg"]
        )
        for k, v in actual_data_gen:  # NOTE, DO NOT use dict() to fetch all the items
            actual_data = v
            expect_data = keydata.get(k, {})
            self.assertTrue(any(actual_data))
            self.assertDictEqual(actual_data, expect_data)
        # ------- subcase #2, add 5 items and automatically flush
        persist_handler.auto_flush = True
        persist_handler.flush_threshold = 5
        extra_keydata = {
            "crypto-key-id-1%s"
            % idx: {
                "exp": (
                    today + timedelta(days=random.randrange(2, max_expired_after_days))
                ).isoformat(),
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
                "use": "Verify",
            }
            for idx in range(5)
        }
        self.assertEqual(len(keydata.keys()), len(persist_handler))
        for key, item in extra_keydata.items():
            persist_handler[key] = item
        keydata.update(extra_keydata)
        self.assertEqual(len(keydata.keys()), len(persist_handler))
        actual_data_gen = persist_handler.items(
            present_fields=["exp", "kty", "use", "alg"]
        )
        for k, v in actual_data_gen:  # NOTE, DO NOT use dict() to fetch all the items
            actual_data = v
            expect_data = keydata.get(k, {})
            self.assertTrue(any(actual_data))
            self.assertDictEqual(actual_data, expect_data)

    ## end of  test_save_key_items

    def test_set_invalid_items(self):
        persist_handler = JWKSFilePersistHandler(**self._init_kwargs)
        today = date.today()
        keydata = {
            "crypto-key-id-0001": {
                "exp": "invalid_iso_format_date",
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
            }
        }

        def exe_fn():
            persist_handler["crypto-key-id-0001"] = keydata["crypto-key-id-0001"]

        err = capture_error(testcase=self, err_cls=ValueError, exe_fn=exe_fn)
        pos = err.args[0].find("key item should cover the minimum set")
        self.assertGreater(pos, 0)

        keydata["crypto-key-id-0001"]["use"] = "verify\x00"
        err = capture_error(testcase=self, err_cls=ValueError, exe_fn=exe_fn)
        pos = err.args[0].find("Invalid isoformat string")
        self.assertGreaterEqual(pos, 0)

        expired_after_days = persist_handler.max_expired_after_days + 1
        exceed_expiry_date = today + timedelta(days=expired_after_days)
        keydata["crypto-key-id-0001"]["exp"] = exceed_expiry_date.isoformat()
        err = capture_error(testcase=self, err_cls=ValueError, exe_fn=exe_fn)
        expect_err_msg = (
            "user-specified expiry date %s exceeds maximum allowed value"
            % exceed_expiry_date.isoformat()
        )
        pos = err.args[0].find(expect_err_msg)
        self.assertGreaterEqual(pos, 0)

        expired_after_days = persist_handler.max_expired_after_days - 1
        valid_expiry_date = today + timedelta(days=expired_after_days)
        keydata["crypto-key-id-0001"]["exp"] = valid_expiry_date.isoformat()
        err = capture_error(testcase=self, err_cls=ValueError, exe_fn=exe_fn)
        pos = err.args[0].find("the value is not printable")
        self.assertGreaterEqual(pos, 0)

        keydata["crypto-key-id-0001"]["use"] = "Verify"
        exe_fn()

    ## end of test_set_invalid_items()

    def test_mix_add_remove_items(self):
        init_kwargs = self._init_kwargs.copy()
        init_kwargs.update({"flush_threshold": 5, "auto_flush": True})
        persist_handler = JWKSFilePersistHandler(**init_kwargs)
        max_expired_after_days = persist_handler.max_expired_after_days
        today = date.today()
        keydata = {
            "crypto-key-id-%s"
            % idx: {
                "exp": (
                    today + timedelta(days=random.randrange(2, max_expired_after_days))
                ).isoformat(),
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
                "use": "Signature",
            }
            for idx in range(init_kwargs["flush_threshold"])
        }
        for key, item in keydata.items():
            persist_handler[key] = item
        extra_keydata = {
            "crypto-key-id-000%s"
            % idx: {
                "exp": (
                    today + timedelta(days=random.randrange(2, max_expired_after_days))
                ).isoformat(),
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
                "use": "Verify",
            }
            for idx in range(3)
        }
        remove_key_ids = ("crypto-key-id-4", "crypto-key-id-2")
        dup_key_id = remove_key_ids[0]
        extra_keydata[dup_key_id] = extra_keydata.pop("crypto-key-id-0001")
        persist_handler.remove(key_ids=remove_key_ids[1:])
        for key, item in extra_keydata.items():
            persist_handler[key] = item

        tuple(map(lambda kid: keydata.pop(kid, None), remove_key_ids))
        keydata = keydata | extra_keydata
        self.assertEqual(len(keydata.keys()), len(persist_handler))
        actual_data_gen = persist_handler.items(
            present_fields=["exp", "kty", "use", "alg"]
        )
        for k, v in actual_data_gen:  # NOTE, DO NOT use dict() to fetch all the items
            actual_data = v
            expect_data = keydata.get(k, {})
            self.assertTrue(any(actual_data))
            self.assertDictEqual(actual_data, expect_data)

    def test_random_choose(self):
        init_kwargs = self._init_kwargs.copy()
        init_kwargs.update({"flush_threshold": 5, "auto_flush": True})
        persist_handler = JWKSFilePersistHandler(**init_kwargs)
        max_expired_after_days = persist_handler.max_expired_after_days
        today = date.today()
        keydata = {
            "crypto-key-id-%s"
            % idx: {
                "exp": (
                    today + timedelta(days=random.randrange(2, max_expired_after_days))
                ).isoformat(),
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
                "use": "Signature",
            }
            for idx in range(init_kwargs["flush_threshold"])
        }
        for key, item in keydata.items():
            persist_handler[key] = item
        expect_kids = set(keydata.keys())
        actual_kids = set()
        for _ in range(500):  # ensure all keys have been chosen at least once
            keyitem = persist_handler.random_choose()
            actual_kids.add(keyitem["kid"])
        self.assertSetEqual(expect_kids, actual_kids)

    def test_evict_expired_keys(self):
        today = date.today()
        init_kwargs = self._init_kwargs.copy()
        init_kwargs.update({"flush_threshold": 10, "auto_flush": True})
        persist_handler = JWKSFilePersistHandler(**init_kwargs)
        max_expired_after_days = persist_handler.max_expired_after_days
        keydata = {
            "crypto-key-id-%s"
            % idx: {
                "exp": (today + timedelta(days=random.randrange(1, 20))).isoformat(),
                "alg": "ALGORITHM_OPTION_%s" % random.randrange(0x1000, 0xFFFF),
                "kty": "CRYPTO_KEY_TYPE_%s" % random.randrange(0x1000, 0xFFFF),
                "use": "Signature",
            }
            for idx in range(init_kwargs["flush_threshold"])
        }
        for key, item in keydata.items():
            persist_handler[key] = item
        self.assertEqual(len(keydata.keys()), len(persist_handler))
        date_limit = today + timedelta(days=8)
        persist_handler.evict_expired_keys(date_limit=date_limit)
        persist_handler.flush()
        keydata = dict(
            filter(
                lambda kv: date.fromisoformat(kv[1]["exp"]) >= date_limit,
                keydata.items(),
            )
        )
        self.assertEqual(len(keydata.keys()), len(persist_handler))
        actual_data_gen = persist_handler.items(
            present_fields=["exp", "kty", "use", "alg"]
        )
        for k, v in actual_data_gen:
            actual_data = v
            expect_data = keydata.get(k, {})
            self.assertTrue(any(actual_data))
            self.assertDictEqual(actual_data, expect_data)


## end of class FilePersistHandlerTestCase
