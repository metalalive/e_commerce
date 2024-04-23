import math
import random
import unittest

from ecommerce_common.auth.keystore import RSAKeygenHandler
from ecommerce_common.auth.jwt import JwkRsaKeygenHandler
from ecommerce_common.tests.common import capture_error


def modInverse(a, m):
    # this function comes from https://www.geeksforgeeks.org/multiplicative-inverse-under-modulo-m/
    m0 = m
    y = 0
    x = 1
    if m == 1:
        return 0
    while a > 1:
        # q is quotient
        q = a // m
        t = m
        # m is remainder now, process
        # same as Euclid's algo
        m = a % m
        a = t
        t = y
        # Update x and y
        y = x - q * y
        x = t
    # Make x positive
    if x < 0:
        x = x + m0
    return x


class RSAkeygenTestCase(unittest.TestCase):
    def setUp(self):
        handler = RSAKeygenHandler()

        def exe_fn(key_size, num_primes):
            return handler.generate(key_size_in_bits=key_size, num_primes=num_primes)

        self._exe_fn = exe_fn

    def tearDown(self):
        pass

    def test_fail(self):
        err = capture_error(
            testcase=self,
            err_cls=ValueError,
            exe_fn=self._exe_fn,
            exe_kwargs={"key_size": 1023, "num_primes": 2},
        )
        pos = err.args[0].find("number of bits has to range between")
        self.assertGreaterEqual(pos, 0)
        err = capture_error(
            testcase=self,
            err_cls=ValueError,
            exe_fn=self._exe_fn,
            exe_kwargs={"key_size": 1024, "num_primes": 1},
        )
        pos = err.args[0].find("number of primes must be at least 2")
        self.assertGreaterEqual(pos, 0)

    def test_ok(self):
        result = self._exe_fn(key_size=1024, num_primes=2)
        expect_key_types = {
            "public": ["e", "n"],
            "private": ["dq", "e", "qp", "d", "dp", "q", "p", "n"],
        }
        for key_type, elements_label in expect_key_types.items():
            key = result[key_type]
            for elm_label in elements_label:
                elm = key[elm_label]
                self.assertTrue(elm.isdigit())
                key[elm_label] = int(elm)
        expect_n = result["private"]["q"] * result["private"]["p"]
        actual_n = result["public"]["n"]
        self.assertEqual(actual_n, expect_n)
        p_minus_one = result["private"]["p"] - 1
        q_minus_one = result["private"]["q"] - 1
        lcm_n = math.lcm(p_minus_one, q_minus_one)
        actual_value = math.gcd(lcm_n, result["public"]["e"])
        self.assertEqual(actual_value, 1)
        ## TODO, sometimes the 2 values failed to match, figure out the reason
        ##expect_d = modInverse(a=result['public']['e'], m=lcm_n)
        ##actual_d = result['private']['d']
        ##self.assertEqual(actual_d, expect_d)


class JwkRSAkeygenTestCase(unittest.TestCase):
    def setUp(self):
        self.handler = JwkRsaKeygenHandler()

    def tearDown(self):
        pass

    def test_ok(self):
        key_size_in_bits = 2048
        keyset = self.handler.generate(key_size_in_bits=key_size_in_bits)
        self.assertEqual(keyset.size, key_size_in_bits)
        key_content = {"public": {}, "private": {}}
        keyset.private(key_content["private"])
        keyset.public(key_content["public"])
        self.assertTrue(any(key_content["private"]))
        self.assertTrue(any(key_content["public"]))
        expect_key_types = {
            "public": ["e", "n"],
            "private": ["dq", "e", "qi", "d", "dp", "q", "p", "n"],
        }
        for key_type, elements_label in expect_key_types.items():
            key = key_content[key_type]
            for elm_label in elements_label:
                elm = key.get(elm_label)
                self.assertIsNotNone(elm)
