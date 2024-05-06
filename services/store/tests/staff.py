import random
from unittest.mock import patch

import pytest

# load the module `tests.common` first, to ensure all environment variables
# are properly set
from tests.common import (
    db_engine_resource,
    session_for_test,
    session_for_setup,
    keystore,
    test_client,
    store_data,
    email_data,
    phone_data,
    loc_data,
    opendays_data,
    staff_data,
    product_avail_data,
    saved_store_objs,
)

from ecommerce_common.models.constants import ROLE_ID_STAFF
from ecommerce_common.models.enums.base import AppCodeOptions, ActivationStatus
from ecommerce_common.util.messaging.rpc import RpcReplyEvent

from store.models import StoreStaff

app_code = AppCodeOptions.store.value[0]


class TestUpdate:
    url = "/profile/{store_id}/staff"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [
            {
                "app_code": app_code,
                "mat_code": StoreStaff.quota_material.value,
                "maxnum": -1,
            }
        ],
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
    def test_ok(
        self, session_for_test, keystore, test_client, saved_store_objs, staff_data
    ):
        num_new, num_unmodified = 1, 3
        obj = next(saved_store_objs)
        body = [
            {
                "staff_id": s.staff_id,
                "start_after": s.start_after.astimezone().isoformat(),
                "end_before": s.end_before.astimezone().isoformat(),
            }
            for s in obj.staff[num_new:]
        ]
        new_staff_d = [next(staff_data) for _ in range(num_unmodified)]
        for item in new_staff_d:
            item["start_after"] = item["start_after"].isoformat()
            item["end_before"] = item["end_before"].isoformat()
        body.extend(new_staff_d)
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.supervisor_id
        auth_data["quotas"][0]["maxnum"] = len(obj.staff) + num_new
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body["result"] = list(map(lambda d: d["staff_id"], body))
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 200
        query = session_for_test.query(StoreStaff).filter(StoreStaff.store_id == obj.id)
        query = query.order_by(StoreStaff.staff_id.asc())

        extra_expect_items = [
            {
                "staff_id": s.staff_id,
                "start_after": s.start_after.astimezone().isoformat(),
                "end_before": s.end_before.astimezone().isoformat(),
            }
            for s in obj.staff[:num_new]
        ]
        body.extend(extra_expect_items)
        expect_value = sorted(body, key=lambda d: d["staff_id"])
        actual_value = list(map(lambda obj: obj.__dict__, query.all()))
        for item in actual_value:
            item.pop("_sa_instance_state", None)
            item.pop("store_id", None)
            item["start_after"] = item["start_after"].astimezone().isoformat()
            item["end_before"] = item["end_before"].astimezone().isoformat()
        assert expect_value == actual_value

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    def test_invalid_staff_id(
        self, session_for_test, keystore, test_client, saved_store_objs
    ):
        obj = next(saved_store_objs)
        body = [
            {
                "staff_id": s.staff_id,
                "start_after": s.start_after.isoformat(),
                "end_before": s.end_before.isoformat(),
            }
            for s in obj.staff
        ]
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.supervisor_id
        auth_data["quotas"][0]["maxnum"] = len(body)
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body["status"] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body["result"] = list(map(lambda d: d["staff_id"], body[2:]))
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        assert result["detail"]["code"] == "invalid_descendant"
        assert result["detail"]["supervisor_id"] == obj.supervisor_id
        expect_invalid_staff = set(map(lambda d: d["staff_id"], body[:2]))
        actual_invalid_staff = set(result["detail"]["staff_ids"])
        assert expect_invalid_staff == actual_invalid_staff
