import random
from unittest.mock import patch

import pytest
from sqlalchemy import select as sa_select
from sqlalchemy.orm import selectinload

from ecommerce_common.models.constants import ROLE_ID_STAFF
from ecommerce_common.models.enums.base import AppCodeOptions, ActivationStatus
from ecommerce_common.util.messaging.rpc import RpcReplyEvent

from store.models import StoreProfile, StorePhone, StoreEmail, StoreCurrency
from store.api.rpc import _get_shop_profile

app_code = AppCodeOptions.store.value[0]


class TestCreation:
    # class name must start with TestXxxx
    url = "/profiles"

    def test_auth_failure(self, keystore, test_client):
        # no need to test CORS middleware ?
        response = test_client.post(self.url, headers={}, json=[])
        assert response.status_code == 401
        response = test_client.post(
            self.url, headers={"Authorization": "Bearer abc1234efg"}, json=[]
        )
        assert response.status_code == 401
        result = response.json()
        profile_data = {
            "id": 136,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_xoxoxox"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.post(
                self.url,
                headers={"Authorization": "Bearer %s" % encoded_token},
                json=[],
            )
        assert response.status_code == 403
        result = response.json()
        assert result["detail"] == "Permission check failure"

    def _mocked_rpc_reply_refresh(self, *args, **kwargs):
        # skip receiving message from RPC-reply-queue
        pass

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_bulk_ok(
        self,
        session_for_test,
        keystore,
        test_client,
        store_data,
        email_data,
        phone_data,
        loc_data,
    ):
        num_items = 4
        profile_data = {
            "id": 130,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = [next(store_data) for _ in range(num_items)]
        for item in body:
            item["emails"] = [next(email_data) for _ in range(random.randrange(0, 3))]
            item["phones"] = [next(phone_data) for _ in range(random.randrange(0, 3))]
            if random.choice([True, False]):
                _loc_data = next(loc_data)
                _loc_data["country"] = _loc_data["country"].value
                item["location"] = _loc_data
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                reply_event = RpcReplyEvent(listener=self, timeout_s=7)
                reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
                reply_event.resp_body["result"] = [
                    {
                        "id": item["supervisor_id"],
                        "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                        "quota": [
                            {
                                "app_code": app_code,
                                "mat_code": StoreProfile.quota_material.value,
                                "maxnum": random.randrange(3, 6),
                            },
                            {
                                "app_code": app_code,
                                "mat_code": StoreEmail.quota_material.value,
                                "maxnum": random.randrange(3, 6),
                            },
                            {
                                "app_code": app_code,
                                "mat_code": StorePhone.quota_material.value,
                                "maxnum": random.randrange(3, 6),
                            },
                        ],
                    }
                    for item in body
                ]
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 201
        result = response.json()
        expect_prof_ids = list(map(lambda d: d["supervisor_id"], body))
        stmt = sa_select(StoreProfile.id, StoreProfile.supervisor_id).filter(
            StoreProfile.supervisor_id.in_(expect_prof_ids)
        )
        resultset = await session_for_test.execute(stmt)
        expect_data = dict(resultset.all())
        actual_data = dict(map(lambda d: (d["id"], d["supervisor_id"]), result))
        assert expect_data == actual_data

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_empty_input(self, keystore, test_client):
        profile_data = {
            "id": 58,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch("ecommerce_common.util.messaging.rpc.MethodProxy._call"):
                body = []
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 422
        result = response.json()
        assert "Empty request body Not Allowed" in result["detail"][0]["msg"]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch("ecommerce_common.util.messaging.rpc.MethodProxy._call"):
                body = [{}, {}]
                response = test_client.post(self.url, headers=headers, json=body)
        result = response.json()
        assert response.status_code == 422
        for item in result["detail"]:
            assert item["loc"][-1] in ("label", "supervisor_id", "currency")
            assert item["msg"] == "Field required"

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_auth_app_down(self, keystore, test_client, store_data):
        profile_data = {
            "id": 58,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = [next(store_data)]
        expect_rpc_fail_status = [
            RpcReplyEvent.status_opt.FAIL_CONN,
            RpcReplyEvent.status_opt.FAIL_PUBLISH,
            RpcReplyEvent.status_opt.REMOTE_ERROR,
        ]
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["result"] = []
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                for fail_status in expect_rpc_fail_status:
                    reply_event.resp_body["status"] = fail_status
                    mocked_rpc_proxy_call.return_value = reply_event
                    response = test_client.post(self.url, headers=headers, json=body)
                    result = response.json()
                    assert response.status_code == 503
                    assert result["detail"] == {
                        "app_code": [AppCodeOptions.user_management.value[0]]
                    }

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_invalid_supervisor_id(self, keystore, test_client, store_data):
        num_items = 4
        profile_data = {
            "id": 99,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = [next(store_data) for _ in range(num_items)]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                reply_event = RpcReplyEvent(listener=self, timeout_s=7)
                reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
                mock_rpc_result = [
                    {
                        "id": item["supervisor_id"],
                        "quota": [],
                        "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                    }
                    for item in body[1:]
                ]
                mock_rpc_result[0]["auth"] = ActivationStatus.ACCOUNT_NON_EXISTENT.value
                reply_event.resp_body["result"] = mock_rpc_result
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        expect_data = {
            0: {"non-existent user profile"},
            1: {"unable to login"},
        }
        for body_idx, expect_value in expect_data.items():
            actual_value = result["detail"][body_idx]["supervisor_id"]
            assert expect_value == set(actual_value)

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_invalid_email(self, keystore, test_client, store_data, email_data):
        num_stores = 2
        invalid_emails = [
            "xyz@ur873",
            "alg0exp3rt.\x05O",
            "Alg0@expat@AiOoh",
            None,
            "",
            "xutye",
        ]
        profile_data = {
            "id": 96,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = [next(store_data) for _ in range(num_stores)]
        body[0]["emails"] = [{"addr": addr} for addr in invalid_emails]
        body[1]["emails"] = [next(email_data) for _ in range(2)]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                reply_event = RpcReplyEvent(listener=self, timeout_s=7)
                reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
                reply_event.resp_body["result"] = [
                    {
                        "id": item["supervisor_id"],
                        "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                        "quota": [
                            {
                                "app_code": app_code,
                                "mat_code": StoreProfile.quota_material.value,
                                "maxnum": num_stores,
                            },
                            {
                                "app_code": app_code,
                                "mat_code": StoreEmail.quota_material.value,
                                "maxnum": len(invalid_emails),
                            },
                        ],
                    }
                    for item in body
                ]
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 422
        result = response.json()
        for err in result["detail"]:
            loc_tail = err["loc"][-4:]
            assert (
                loc_tail[0] == 0 and loc_tail[1] == "emails" and loc_tail[3] == "addr"
            )
            assert (
                "value is not a valid email address" in err["msg"]
                or "Input should be a valid string" in err["msg"]
            )

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_invalid_phone(self, keystore, test_client, store_data, phone_data):
        num_stores = 2
        invalid_phones = [
            (None, 3415),
            (-4, 392947104824833530),
            ("6ob", "88934\x014762"),
            ("", "3-40-019"),
        ]
        profile_data = {
            "id": 71,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = [next(store_data) for _ in range(num_stores)]
        body[0]["phones"] = [next(phone_data) for _ in range(2)]
        body[1]["phones"] = [
            {"country_code": phone[0], "line_number": phone[1]}
            for phone in invalid_phones
        ]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                reply_event = RpcReplyEvent(listener=self, timeout_s=7)
                reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
                reply_event.resp_body["result"] = [
                    {
                        "id": item["supervisor_id"],
                        "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                        "quota": [
                            {
                                "app_code": app_code,
                                "mat_code": StoreProfile.quota_material.value,
                                "maxnum": num_stores,
                            },
                            {
                                "app_code": app_code,
                                "mat_code": StorePhone.quota_material.value,
                                "maxnum": len(invalid_phones),
                            },
                        ],
                    }
                    for item in body
                ]
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 422
        result = response.json()
        for err in result["detail"]:
            assert (
                err["msg"] == "Input should be a valid string"
                or "should match pattern" in err["msg"]
            )
            assert err["loc"][2] == "phones"
            assert err["loc"][-1] in (
                "line_number",
                "country_code",
            )

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_quota_limit_exceeds(self, keystore, test_client, store_data):
        num_stores_saved, num_new_stores, max_num_stores_per_user = 3, 2, 4
        profile_data = {
            "id": 71,
            "privilege_status": ROLE_ID_STAFF,
            "quotas": [],
            "roles": [
                {"app_code": app_code, "codename": "view_storeprofile"},
                {"app_code": app_code, "codename": "add_storeprofile"},
            ],
        }
        encoded_token = keystore.gen_access_token(
            profile=profile_data, audience=["store"]
        )
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = [next(store_data) for _ in range(num_stores_saved)]
        chosen_supervisor_id = body[0]["supervisor_id"]
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body["result"] = [
            {
                "id": item["supervisor_id"],
                "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                "quota": [
                    {
                        "app_code": app_code,
                        "mat_code": StoreProfile.quota_material.value,
                        "maxnum": max_num_stores_per_user,
                    }
                ],
            }
            for item in body
        ]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
                assert response.status_code == 201
                body = [next(store_data) for _ in range(num_new_stores)]
                for item in body:
                    item["supervisor_id"] = chosen_supervisor_id
                response = test_client.post(self.url, headers=headers, json=body)
                assert response.status_code == 201
                response = test_client.post(self.url, headers=headers, json=body)
                assert response.status_code == 403
                result = response.json()
                assert len(result["detail"]) == num_new_stores
                err = result["detail"][0]
                assert err["supervisor_id"] == chosen_supervisor_id
                assert err["store_profile"]["type"] == "limit-exceed"
                assert err["store_profile"]["max_limit"] == max_num_stores_per_user
                assert err["store_profile"]["num_existing_items"] == num_stores_saved
                assert err["store_profile"]["num_new_items"] == num_new_stores


class TestUpdateContact:
    # class name must start with TestXxxx
    url = "/profile/{store_id}"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [],
        "roles": [
            {"app_code": app_code, "codename": "view_storeprofile"},
            {"app_code": app_code, "codename": "change_storeprofile"},
        ],
    }

    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(
        self,
        session_for_verify,
        keystore,
        test_client,
        saved_store_objs,
        email_data,
        phone_data,
        loc_data,
    ):
        obj = await anext(saved_store_objs)
        body = {
            "label": "edited_label",
            "active": not obj.active,
            "currency": StoreCurrency.TWD.value,
        }
        body["emails"] = list(map(lambda e: {"addr": e.addr}, obj.emails[1:]))
        body["phones"] = list(
            map(
                lambda e: {
                    "country_code": e.country_code,
                    "line_number": e.line_number,
                },
                obj.phones[1:],
            )
        )
        body["emails"].append(next(email_data))
        body["phones"].append(next(phone_data))
        body["location"] = next(loc_data)
        body["location"]["country"] = body["location"]["country"].value
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.supervisor_id
        auth_data["quotas"] = [
            {
                "app_code": app_code,
                "mat_code": StoreEmail.quota_material.value,
                "maxnum": len(body["emails"]),
            },
            {
                "app_code": app_code,
                "mat_code": StorePhone.quota_material.value,
                "maxnum": len(body["phones"]),
            },
        ]
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 200
        stmt = (
            sa_select(StoreProfile)
            .filter(StoreProfile.id == obj.id)
            .options(
                selectinload(StoreProfile.emails),
                selectinload(StoreProfile.phones),
                selectinload(StoreProfile.location),
            )
        )
        resultset = await session_for_verify.execute(stmt)
        row = resultset.first()
        obj = row[0]
        assert obj.label == body["label"]
        assert obj.active == body["active"]
        expect_value = set(map(lambda e: e.addr, obj.emails))
        actual_value = set(map(lambda e: e["addr"], body["emails"]))
        assert expect_value == actual_value
        expect_value = set(map(lambda p: (p.country_code, p.line_number), obj.phones))
        actual_value = set(
            map(lambda p: (p["country_code"], p["line_number"]), body["phones"])
        )
        assert expect_value == actual_value
        assert obj.location.country.value == body["location"]["country"]
        for col_name in ("locality", "street", "detail", "floor"):
            assert getattr(obj.location, col_name) == body["location"][col_name]

    @pytest.mark.asyncio(loop_scope="session")
    async def test_quota_limit_exceeds(
        self,
        keystore,
        test_client,
        saved_store_objs,
        email_data,
        phone_data,
    ):
        max_num_emails = 4
        max_num_phones = 5
        obj = await anext(saved_store_objs)
        body = {
            "label": "edited_label",
            "active": not obj.active,
            "currency": StoreCurrency.INR.value,
        }
        body["emails"] = [next(email_data) for _ in range(max_num_emails + 1)]
        body["phones"] = [next(phone_data) for _ in range(max_num_phones + 1)]
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.supervisor_id
        auth_data["quotas"] = [
            {
                "app_code": app_code,
                "mat_code": StoreEmail.quota_material.value,
                "maxnum": max_num_emails,
            },
            {
                "app_code": app_code,
                "mat_code": StorePhone.quota_material.value,
                "maxnum": max_num_phones,
            },
        ]
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 403
        result = response.json()
        assert result["detail"]["emails"][0].startswith("Limit exceeds")
        body["emails"].pop()
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 403
        result = response.json()
        assert result["detail"]["phones"][0].startswith("Limit exceeds")

    def test_invalid_id(self, keystore, test_client):
        invalid_supervisor_id = -9876
        body = {
            "label": "edited label",
            "active": True,
            "currency": StoreCurrency.THB.value,
        }
        auth_data = self._auth_data_pattern
        auth_data["id"] = invalid_supervisor_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            invalid_store_id = "b99"
            url = self.url.format(store_id=invalid_store_id)
            response = test_client.patch(url, headers=headers, json=body)
            assert response.status_code == 422
            invalid_store_id = -998
            url = self.url.format(store_id=invalid_store_id)
            response = test_client.patch(url, headers=headers, json=body)
            assert response.status_code == 422
            invalid_store_id = (
                1  # there should not be any store profile in database in the test case
            )
            url = self.url.format(store_id=invalid_store_id)
            response = test_client.patch(url, headers=headers, json=body)
            assert response.status_code == 404
            assert response.json()["detail"] == {"code": "not_exist"}

    @pytest.mark.asyncio(loop_scope="session")
    async def test_invalid_supervisor(self, keystore, test_client, saved_store_objs):
        obj = await anext(saved_store_objs)
        invalid_supervisor_id = obj.supervisor_id + 9999
        body = {
            "label": "edited label",
            "active": not obj.active,
            "currency": StoreCurrency.IDR.value,
        }
        auth_data = self._auth_data_pattern
        auth_data["id"] = invalid_supervisor_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            url = self.url.format(store_id=obj.id)
            response = test_client.patch(url, headers=headers, json=body)
            assert response.status_code == 403
            assert response.json()["detail"] == "Not allowed to edit the store profile"


class TestSwitchSupervisor:
    url = "/profile/{store_id}/supervisor"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [],
        "roles": [
            {"app_code": app_code, "codename": "view_storeprofile"},
            {"app_code": app_code, "codename": "change_storeprofile"},
        ],
    }

    def _mocked_rpc_reply_refresh(self, *args, **kwargs):
        # skip receiving message from RPC-reply-queue
        pass

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(
        self, session_for_verify, keystore, test_client, saved_store_objs
    ):
        obj = await anext(saved_store_objs)
        old_supervisor_id = obj.supervisor_id
        new_supervisor_id = 5566
        auth_data = self._auth_data_pattern
        auth_data["id"] = old_supervisor_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = {"supervisor_id": new_supervisor_id}
        url = self.url.format(store_id=obj.id)
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body["result"] = [
            {
                "id": new_supervisor_id,
                "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                "quota": [
                    {
                        "app_code": app_code,
                        "mat_code": StoreProfile.quota_material.value,
                        "maxnum": random.randrange(3, 6),
                    }
                ],
            }
        ]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 200
        stmt = sa_select(StoreProfile).filter(StoreProfile.id == obj.id)
        resultset = await session_for_verify.execute(stmt)
        row = resultset.first()
        obj = row[0]
        assert obj.supervisor_id == new_supervisor_id

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_quota_limit_exceeds(
        self, session_for_verify, keystore, test_client, saved_store_objs, store_data
    ):
        max_num_stores = 5
        objs = [await anext(saved_store_objs) for _ in range(max_num_stores)]
        new_supervisor_id = 5567
        body = {"supervisor_id": new_supervisor_id}
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body["result"] = [
            {
                "id": new_supervisor_id,
                "auth": ActivationStatus.ACCOUNT_ACTIVATED.value,
                "quota": [
                    {
                        "app_code": app_code,
                        "mat_code": StoreProfile.quota_material.value,
                        "maxnum": (max_num_stores - 1),
                    }
                ],
            }
        ]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                for obj in objs:
                    old_supervisor_id = obj.supervisor_id
                    auth_data = self._auth_data_pattern
                    auth_data["id"] = old_supervisor_id
                    encoded_token = keystore.gen_access_token(
                        profile=auth_data, audience=["store"]
                    )
                    headers = {"Authorization": "Bearer %s" % encoded_token}
                    url = self.url.format(store_id=obj.id)
                    response = test_client.patch(url, headers=headers, json=body)
                    expect_status_code = 403 if obj is objs[-1] else 200
                    assert response.status_code == expect_status_code
        stmt = sa_select(StoreProfile.id).filter(
            StoreProfile.supervisor_id == new_supervisor_id
        )
        resultset = await session_for_verify.execute(stmt)
        actual_data = set(map(lambda row: row[0], resultset.fetchall()))
        expect_data = set(map(lambda obj: obj.id, objs[:-1]))
        assert expect_data == actual_data

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_deactivated_supervisor(
        self, keystore, test_client, saved_store_objs
    ):
        obj = await anext(saved_store_objs)
        old_supervisor_id = obj.supervisor_id
        new_supervisor_id = 5568
        auth_data = self._auth_data_pattern
        auth_data["id"] = old_supervisor_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        body = {"supervisor_id": new_supervisor_id}
        url = self.url.format(store_id=obj.id)
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body["result"] = [
            {
                "id": new_supervisor_id,
                "quota": [],
                "auth": ActivationStatus.ACCOUNT_DEACTIVATED.value,
            }
        ]
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        assert result["detail"]["supervisor_id"][0] == "unable to login"


class TestDeletion:
    url = "/profiles?ids=%s"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [],
        "roles": [
            {"app_code": app_code, "codename": "view_storeprofile"},
            {"app_code": app_code, "codename": "delete_storeprofile"},
        ],
    }

    @pytest.mark.asyncio(loop_scope="session")
    async def test_bulk_ok(self, keystore, test_client, saved_store_objs):
        num_items = 7
        num_deleting = 4
        objs = [await anext(saved_store_objs) for _ in range(num_items)]
        auth_data = self._auth_data_pattern
        auth_data["id"] = 214
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        deleting_ids = random.sample(list(map(lambda obj: obj.id, objs)), num_deleting)
        uri_renderred = self.url % ",".join(map(str, deleting_ids))
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            reply_evt_order = RpcReplyEvent(listener=self, timeout_s=1)
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # this endpoint will interact with 2 different services, send the
                # reply events in the order which is acceptable to the backend
                mocked_rpc_proxy_call.return_value = reply_evt_order
                response = test_client.delete(uri_renderred, headers=headers)
                assert response.status_code == 204
                assert mocked_rpc_proxy_call.call_count == num_deleting
                response = test_client.delete(uri_renderred, headers=headers)
                assert response.status_code == 410
                assert mocked_rpc_proxy_call.call_count == (num_deleting << 1)


class TestReadWeb:
    url = "/profile/{store_id}"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [],
        "roles": [
            {"app_code": app_code, "codename": "view_storeprofile"},
        ],
    }

    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, keystore, test_client, saved_store_objs):
        obj = await anext(saved_store_objs)
        url = self.url.format(store_id=obj.id)
        auth_data = self._auth_data_pattern
        # subcase 1, outsider access will be denied
        auth_data["id"] = 56789
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.get(url, headers=headers)
        assert response.status_code == 403
        # subcase 2, valid staff access
        auth_data["id"] = obj.staff[-1].staff_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.get(url, headers=headers)
        assert response.status_code == 200
        result = response.json()
        for field in ("label", "supervisor_id", "active"):
            assert getattr(obj, field) == result[field]
        expect_data = list(
            map(
                lambda obj: {
                    k: getattr(obj, k) for k in ("country_code", "line_number")
                },
                obj.phones,
            )
        )
        actual_data = result["phones"]
        assert expect_data == actual_data
        expect_data = list(
            map(
                lambda obj: {
                    k: getattr(obj, k)
                    for k in ("staff_id", "start_after", "end_before")
                },
                obj.staff,
            )
        )
        for item in expect_data:
            item["start_after"] = item["start_after"].isoformat()
            item["end_before"] = item["end_before"].isoformat()
        actual_data = result["staff"]
        assert expect_data == actual_data
        for field in ("locality", "street", "detail", "floor"):
            assert result["location"][field] == getattr(obj.location, field)


class TestReadRpc:
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, db_engine_resource, saved_store_objs):
        obj = await anext(saved_store_objs)
        actual = await _get_shop_profile(db_engine=db_engine_resource, sid=obj.id)
        assert actual["label"] == obj.label
        assert actual["active"] == obj.active
        assert actual["supervisor_id"] == obj.supervisor_id
        assert len(actual["emails"]) == len(obj.emails)
        assert len(actual["phones"]) == len(obj.phones)
        assert actual["location"]["country"] == obj.location.country
        assert len(actual["staff"]) == len(obj.staff)
        assert len(actual["open_days"]) == len(obj.open_days)
