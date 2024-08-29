import random
from unittest.mock import patch

import pytest
from sqlalchemy import select as sa_select

# load the module `tests.common` first, to ensure all environment variables
# are properly set
from tests.common import (
    db_engine_resource,
    session_for_test,
    session_for_verify,
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

from store.models import HourOfOperation

app_code = AppCodeOptions.store.value[0]


class TestUpdate:
    url = "/profile/{store_id}/business_hours"
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

    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(
        self, session_for_verify, keystore, test_client, saved_store_objs, opendays_data
    ):
        obj = await anext(saved_store_objs)
        num_items = 3
        body = [next(opendays_data) for _ in range(num_items)]
        for item in body:
            item["day"] = item["day"].value
            item["time_open"] = item["time_open"].isoformat()
            item["time_close"] = item["time_close"].isoformat()
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.supervisor_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 200
        stmt = sa_select(HourOfOperation).filter(HourOfOperation.store_id == obj.id)
        resultset = await session_for_verify.execute(stmt)
        actual_value = list(map(lambda row: row[0].__dict__, resultset.all()))
        for item in actual_value:
            item.pop("_sa_instance_state", None)
            item.pop("store_id", None)
            item["day"] = item["day"].value
            item["time_open"] = item["time_open"].isoformat()
            item["time_close"] = item["time_close"].isoformat()
        expect_value = sorted(body, key=lambda d: d["day"])
        actual_value = sorted(actual_value, key=lambda d: d["day"])
        assert expect_value == actual_value

    @pytest.mark.asyncio(loop_scope="session")
    async def test_duplicate_days(
        self, keystore, test_client, saved_store_objs, opendays_data
    ):
        obj = await anext(saved_store_objs)
        num_items = 5
        body = [next(opendays_data) for _ in range(num_items)]
        for item in body:
            item["day"] = item["day"].value
            item["time_open"] = item["time_open"].isoformat()
            item["time_close"] = item["time_close"].isoformat()
        body[-2]["day"] = body[-1]["day"]
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.supervisor_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        assert result["detail"]["code"] == "duplicate"
        assert "day" in result["detail"]["field"]
