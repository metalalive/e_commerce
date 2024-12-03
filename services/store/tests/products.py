import random
from datetime import datetime
from typing import List, Dict, Iterable
from unittest.mock import MagicMock
from unittest.mock import patch

import pytest
from sqlalchemy import select as sa_select

from ecommerce_common.models.constants import ROLE_ID_STAFF
from ecommerce_common.models.enums.base import AppCodeOptions
from ecommerce_common.util.messaging.rpc import RpcReplyEvent

from store.models import SaleableTypeEnum, StoreProductAvailable

from .common import _saved_obj_gen

app_code = AppCodeOptions.store.value[0]


class TestUpdate:
    url = "/profile/{store_id}/products"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [
            {
                "app_code": app_code,
                "mat_code": StoreProductAvailable.quota_material.value,
                "maxnum": -1,
            }
        ],
        "roles": [
            {"app_code": app_code, "codename": "add_storeproductavailable"},
            {"app_code": app_code, "codename": "change_storeproductavailable"},
            {"app_code": app_code, "codename": "delete_storeproductavailable"},
        ],
    }

    def _mocked_rpc_reply_refresh(self, *args, **kwargs):
        # skip receiving message from RPC-reply-queue
        pass

    def _setup_mock_rpc_reply(
        self, body, timeout_sec=7, status=RpcReplyEvent.status_opt.SUCCESS
    ):
        # mock reply from `product` service
        sale_items_d = filter(
            lambda d: d["product_type"] == SaleableTypeEnum.ITEM.value, body
        )
        sale_pkgs_d = filter(
            lambda d: d["product_type"] == SaleableTypeEnum.PACKAGE.value, body
        )
        sale_items_d = map(lambda d: {"id": d["product_id"]}, sale_items_d)
        sale_pkgs_d = map(lambda d: {"id": d["product_id"]}, sale_pkgs_d)
        reply_event = RpcReplyEvent(listener=self, timeout_s=timeout_sec)
        reply_event.resp_body["status"] = status
        reply_event.resp_body["result"] = {
            "item": list(sale_items_d),
            "pkg": list(sale_pkgs_d),
        }
        return reply_event

    def _setup_base_req_body(
        self, objs, product_avail_gen, num_new_items: int = 2
    ) -> List[Dict]:
        body = [
            {
                "product_id": p.product_id,
                "product_type": p.product_type.value,
                "start_after": p.start_after.astimezone().isoformat(),
                "end_before": p.end_before.astimezone().isoformat(),
                "price": p.price,
            }
            for p in objs
        ]
        if product_avail_gen is not None:
            new_product_d = [next(product_avail_gen) for _ in range(num_new_items)]
            for item in new_product_d:
                item["product_type"] = item["product_type"].value
                item["start_after"] = item["start_after"].astimezone().isoformat()
                item["end_before"] = item["end_before"].astimezone().isoformat()
            body.extend(new_product_d)
        return body

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(
        self,
        session_for_verify,
        keystore,
        test_client,
        saved_store_objs,
        product_avail_data,
    ):
        obj = await anext(saved_store_objs)
        num_new, num_unmodified = 2, 2
        body = self._setup_base_req_body(
            objs=obj.products[num_unmodified:],
            product_avail_gen=product_avail_data,
            num_new_items=num_new,
        )
        auth_data = self._auth_data_pattern
        # authorized user can be either supervisor or staff of the store
        auth_data["id"] = obj.staff[-1].staff_id
        auth_data["quotas"][0]["maxnum"] = len(obj.products) + num_new
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            reply_evt_product = self._setup_mock_rpc_reply(body)
            reply_evt_order = RpcReplyEvent(listener=self, timeout_s=1)
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                # this endpoint will interact with 2 different services, send the
                # reply events in the order which is acceptable to the backend
                mocked_rpc_proxy_call.side_effect = [reply_evt_product, reply_evt_order]
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 200
        stmt = (
            sa_select(StoreProductAvailable)
            .filter(StoreProductAvailable.store_id == obj.id)
            .order_by(StoreProductAvailable.product_id.asc())
        )
        resultset = await session_for_verify.execute(stmt)
        expect_value = [
            *body,
            *self._setup_base_req_body(
                objs=obj.products[:num_unmodified],
                product_avail_gen=None,
                num_new_items=0,
            ),
        ]
        expect_value = sorted(expect_value, key=lambda d: d["product_id"])
        actual_value = list(map(lambda row: row[0].__dict__, resultset))
        for item in actual_value:
            item.pop("_sa_instance_state", None)
            item.pop("store_id", None)
            item["product_type"] = item["product_type"].value
            item["start_after"] = item["start_after"].astimezone().isoformat()
            item["end_before"] = item["end_before"].astimezone().isoformat()
        assert expect_value == actual_value

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_invalid_product_id(self, keystore, test_client, saved_store_objs):
        obj = await anext(saved_store_objs)
        body = self._setup_base_req_body(obj.products, None)
        auth_data = self._auth_data_pattern
        auth_data["id"] = obj.staff[0].staff_id
        auth_data["quotas"][0]["maxnum"] = len(body)
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        reply_event = self._setup_mock_rpc_reply(body[1:])
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        assert result["detail"]["code"] == "invalid"
        err_detail = result["detail"]["field"]
        assert err_detail and any(err_detail)
        expect_value = {k: body[0].get(k) for k in ("product_id", "product_type")}
        actual_value = err_detail[0]
        assert expect_value == actual_value

    @patch(
        "ecommerce_common.util.messaging.rpc.RpcReplyEvent.refresh",
        _mocked_rpc_reply_refresh,
    )
    @pytest.mark.asyncio(loop_scope="session")
    async def test_invalid_staff_id(self, keystore, test_client, saved_store_objs):
        invalid_staff_id = 9999
        obj = await anext(saved_store_objs)
        body = self._setup_base_req_body(obj.products, None)
        auth_data = self._auth_data_pattern
        auth_data["id"] = invalid_staff_id
        auth_data["quotas"][0]["maxnum"] = len(body)
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        reply_event = self._setup_mock_rpc_reply(body[:])
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 403

    @pytest.mark.asyncio(loop_scope="session")
    async def test_emit_event_orderapp(self, saved_store_objs, product_avail_data):
        from store.api.web import emit_event_edit_products

        # subcase 1
        expect_store_id, num_new, num_unmodified = 2345, 3, 2
        arg_creating = list(
            map(
                lambda d: StoreProductAvailable(**next(product_avail_data)),
                range(num_new),
            )
        )
        expect_creating = self._setup_base_req_body(
            objs=arg_creating, product_avail_gen=None, num_new_items=num_new
        )
        obj = await anext(saved_store_objs)
        arg_updating = obj.products[:num_unmodified]
        expect_updating = self._setup_base_req_body(
            objs=arg_updating, product_avail_gen=None, num_new_items=num_unmodified
        )
        mocked_rpc = MagicMock()
        mocked_rpc_fn = mocked_rpc.update_store_products
        mocked_evt = mocked_rpc_fn.return_value
        mocked_evt.status_opt.INITED = RpcReplyEvent.status_opt.INITED
        mocked_evt.result = {
            "status": RpcReplyEvent.status_opt.INITED,
            "result": "unit-test",
        }
        emit_event_edit_products(
            expect_store_id,
            rpc_hdlr=mocked_rpc,
            updating=arg_updating,
            creating=arg_creating,
        )
        mocked_rpc_fn.assert_called_once()
        assert expect_store_id == mocked_rpc_fn.call_args.kwargs["s_id"]
        assert not mocked_rpc_fn.call_args.kwargs["rm_all"]
        assert expect_updating == mocked_rpc_fn.call_args.kwargs["updating"]
        assert expect_creating == mocked_rpc_fn.call_args.kwargs["creating"]
        assert mocked_rpc_fn.call_args.kwargs["deleting"].get("items") is None
        assert mocked_rpc_fn.call_args.kwargs["deleting"].get("pkgs") is None
        # subcase 2
        expect_deleting = {"items": [2, 3, 4, 5], "pkgs": [16, 79, 203]}
        emit_event_edit_products(
            expect_store_id, rpc_hdlr=mocked_rpc, deleting=expect_deleting
        )
        assert expect_store_id == mocked_rpc_fn.call_args.kwargs["s_id"]
        assert [] == mocked_rpc_fn.call_args.kwargs["updating"]
        assert [] == mocked_rpc_fn.call_args.kwargs["creating"]
        expect_deleting.update(
            {
                "item_type": SaleableTypeEnum.ITEM.value,
                "pkg_type": SaleableTypeEnum.PACKAGE.value,
            }
        )
        assert expect_deleting == mocked_rpc_fn.call_args.kwargs["deleting"]


## end of class TestUpdate


class TestDiscard:
    url = "/profile/{store_id}/products?pitems={ids1}&ppkgs={ids2}"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [
            {
                "app_code": app_code,
                "mat_code": StoreProductAvailable.quota_material.value,
                "maxnum": -1,
            }
        ],
        "roles": [
            {"app_code": app_code, "codename": "add_storeproductavailable"},
            {"app_code": app_code, "codename": "change_storeproductavailable"},
            {"app_code": app_code, "codename": "delete_storeproductavailable"},
        ],
    }

    def _setup_deleting_items(
        self, products: Iterable[StoreProductAvailable], num_deleting: int
    ):
        def is_pkg_check(d) -> bool:
            return d.product_type is SaleableTypeEnum.PACKAGE

        def is_item_check(d) -> bool:
            return d.product_type is SaleableTypeEnum.ITEM

        def get_prod_id_fn(d) -> int:
            return d.product_id

        prod_item_ids = list(map(get_prod_id_fn, filter(is_item_check, products)))
        prod_pkg_ids = list(map(get_prod_id_fn, filter(is_pkg_check, products)))
        deleting_pitems = prod_item_ids[:num_deleting]
        deleting_ppkgs = prod_pkg_ids[:num_deleting]
        remaining_pitems = [
            (SaleableTypeEnum.ITEM, i)
            for i in prod_item_ids
            if i not in deleting_pitems
        ]
        remaining_ppkgs = [
            (SaleableTypeEnum.PACKAGE, i)
            for i in prod_pkg_ids
            if i not in deleting_ppkgs
        ]
        return (deleting_pitems, deleting_ppkgs, remaining_pitems, remaining_ppkgs)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(
        self,
        session_for_test,
        session_for_verify,
        keystore,
        test_client,
        store_data,
        product_avail_data,
        staff_data,
    ):
        num_deleting, num_total = 2, 20
        generator = _saved_obj_gen(
            store_data_gen=store_data,
            session=session_for_test,
            product_avail_data_gen=product_avail_data,
            staff_data_gen=staff_data,
            num_staff_per_store=1,
            num_products_per_store=num_total,
        )
        obj = await anext(generator)
        auth_data = self._auth_data_pattern.copy()
        auth_data["id"] = obj.staff[0].staff_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        deleting_pitems, deleting_ppkgs, remaining_pitems, remaining_ppkgs = (
            self._setup_deleting_items(obj.products, num_deleting)
        )
        renderred_url = self.url.format(
            store_id=obj.id,
            ids1=",".join(map(str, deleting_pitems)),
            ids2=",".join(map(str, deleting_ppkgs)),
        )
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            reply_evt_order = RpcReplyEvent(listener=self, timeout_s=1)
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_evt_order
                response = test_client.delete(renderred_url, headers=headers)
                assert response.status_code == 204
                response = test_client.delete(renderred_url, headers=headers)
                assert response.status_code == 410

        store_id = obj.id
        session_for_test.expire(obj)

        # SQLALchemy does not seem to allow original session to retrieve up-to-date
        # records after previous deletion.
        stmt = sa_select(
            StoreProductAvailable.product_type, StoreProductAvailable.product_id
        ).where(StoreProductAvailable.store_id == store_id)
        resultset = await session_for_verify.execute(stmt)
        actual_remain = resultset.all()
        expect_remain = [*remaining_ppkgs, *remaining_pitems]
        assert len(actual_remain) == num_total - 2 * num_deleting
        assert set(actual_remain) == set(expect_remain)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_error_emit_event(
        self,
        session_for_test,
        session_for_verify,
        keystore,
        test_client,
        store_data,
        product_avail_data,
        staff_data,
    ):
        num_deleting, num_total = 2, 19
        generator = _saved_obj_gen(
            store_data_gen=store_data,
            session=session_for_test,
            product_avail_data_gen=product_avail_data,
            staff_data_gen=staff_data,
            num_staff_per_store=1,
            num_products_per_store=num_total,
        )
        obj = await anext(generator)
        auth_data = self._auth_data_pattern.copy()
        auth_data["id"] = obj.staff[0].staff_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        deleting_pitems, deleting_ppkgs, remaining_pitems, remaining_ppkgs = (
            self._setup_deleting_items(obj.products, num_deleting)
        )
        renderred_url = self.url.format(
            store_id=obj.id,
            ids1=",".join(map(str, deleting_pitems)),
            ids2=",".join(map(str, deleting_ppkgs)),
        )
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            reply_evt_order = RpcReplyEvent(listener=self, timeout_s=1)
            reply_evt_order.send(
                body={
                    "status": RpcReplyEvent.status_opt.FAIL_PUBLISH,
                    "error": "test-mock",
                }
            )
            with patch(
                "ecommerce_common.util.messaging.rpc.MethodProxy._call"
            ) as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_evt_order
                response = test_client.delete(renderred_url, headers=headers)
                assert response.status_code == 500

        stmt = sa_select(
            StoreProductAvailable.product_type, StoreProductAvailable.product_id
        ).where(StoreProductAvailable.store_id == obj.id)
        resultset = await session_for_verify.execute(stmt)
        actual_remain = resultset.all()
        assert len(actual_remain) == num_total


## end of class TestDiscard:


class TestRead:
    url = "/profile/{store_id}/products"
    _auth_data_pattern = {
        "id": -1,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [],
        "roles": [
            {"app_code": app_code, "codename": "view_storeproductavailable"},
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
        self,
        session_for_test,
        keystore,
        test_client,
        saved_store_objs,
        product_avail_data,
    ):
        obj = await anext(saved_store_objs)
        new_products_avail = [
            StoreProductAvailable(**next(product_avail_data)) for _ in range(20)
        ]
        obj.products.extend(new_products_avail)
        await session_for_test.commit()
        await session_for_test.refresh(obj, attribute_names=["staff"])
        auth_data = self._auth_data_pattern
        auth_data["id"] = random.choice(obj.staff).staff_id
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=["store"])
        headers = {"Authorization": "Bearer %s" % encoded_token}
        url = self.url.format(store_id=obj.id)
        with patch("jwt.PyJWKClient.fetch_data", keystore._mocked_get_jwks):
            response = test_client.get(url, headers=headers)
        assert response.status_code == 200
        result = response.json()
        expect_sale_types = [opt.value for opt in SaleableTypeEnum]
        for item in result:
            assert item.get("product_id") > 0
            assert item.get("product_type") in expect_sale_types
            start_after = datetime.fromisoformat(result[-1]["start_after"])
            end_before = datetime.fromisoformat(result[-1]["end_before"])
            assert start_after < end_before
