import random

from django.test import TransactionTestCase

from product.models.base import ProductAttributeType

from ..common import (
    _fixtures,
    _MockTestClientInfoMixin,
    assert_view_permission_denied,
    assert_view_bulk_create_with_response,
    app_code_product,
    priv_status_staff,
)


class AttrTypeBaseViewTestCase(TransactionTestCase, _MockTestClientInfoMixin):
    def setUp(self):
        self._setup_keystore()

    def tearDown(self):
        self._teardown_keystore()
        self._client.cookies.clear()


## end of class AttrTypeBaseViewTestCase


class AttrTypeBaseViewTestCase(AttrTypeBaseViewTestCase):
    path = "/attrtypes"

    def setUp(self):
        super().setUp()
        self.request_data = [
            random.choice(_fixtures["ProductAttributeType"]) for _ in range(5)
        ]

    def test_permission_denied(self):
        kwargs = {
            "testcase": self,
            "request_body_data": self.request_data,
            "path": self.path,
            "permissions": ["view_productattributetype", "add_productattributetype"],
            "http_method": "post",
        }
        assert_view_permission_denied(**kwargs)
        kwargs["http_method"] = "put"
        kwargs["permissions"] = [
            "view_productattributetype",
            "change_productattributetype",
        ]
        assert_view_permission_denied(**kwargs)

    def test_bulk_ok_with_full_response(self):
        expect_shown_fields = ["id", "name", "dtype"]
        created_attrtypes = assert_view_bulk_create_with_response(
            testcase=self,
            path=self.path,
            method="post",
            body=self.request_data,
            expect_shown_fields=expect_shown_fields,
            expect_hidden_fields=[],
            permissions=["view_productattributetype", "add_productattributetype"],
        )
        created_attrtypes = sorted(created_attrtypes, key=lambda d: d["id"])
        ids = tuple(map(lambda d: d["id"], created_attrtypes))
        qset = ProductAttributeType.objects.filter(id__in=ids).order_by("id")
        expect_value = list(qset.values(*expect_shown_fields))
        actual_value = created_attrtypes
        self.assertListEqual(expect_value, actual_value)

    def test_invalid_dtype(self):
        expect_usrprof = 71
        permissions = ["view_productattributetype", "add_productattributetype"]
        access_tok_payld = {
            "id": expect_usrprof,
            "privilege_status": priv_status_staff,
            "quotas": [],
            "roles": [
                {"app_code": app_code_product, "codename": codename}
                for codename in permissions
            ],
        }
        access_token = self.gen_access_token(
            profile=access_tok_payld, audience=["product"]
        )
        invalid_dtype = "xxoo"
        self.request_data[-1]["dtype"] = invalid_dtype
        response = self._send_request_to_backend(
            path=self.path,
            method="post",
            body=self.request_data,
            access_token=access_token,
        )
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        expect_errmsg = '"%s" is not a valid choice.' % invalid_dtype
        actual_errmsg = err_info[-1]["dtype"][0]
        self.assertEqual(expect_errmsg, actual_errmsg)
