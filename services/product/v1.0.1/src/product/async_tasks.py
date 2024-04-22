import logging
from typing import List

from celery.backends.rpc import RPCBackend as CeleryRpcBackend

from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME
from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

from .serializers.base import SaleableItemSerializer, SaleablePackageSerializer

_logger = logging.getLogger(__name__)


@celery_app.task(
    backend=CeleryRpcBackend(app=celery_app),
    queue="rpc_productmgt_get_product",
    exchange=RPC_EXCHANGE_DEFAULT_NAME,
    routing_key="rpc.product.get_product",
)
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def get_product(
    item_ids: List[int],
    pkg_ids: List[int],
    item_fields: List[str],
    pkg_fields: List[str],
    profile: int,
) -> dict:
    item_ids = list(set(item_ids))
    pkg_ids = list(set(pkg_ids))
    item_fields = list(set(item_fields))
    pkg_fields = list(set(pkg_fields))
    _map = {
        SaleableItemSerializer: {
            "ids": item_ids,
            "fields": item_fields,
            "output_key": "item",
        },
        SaleablePackageSerializer: {
            "ids": pkg_ids,
            "fields": pkg_fields,
            "output_key": "pkg",
        },
    }
    out = {}
    for serializer_cls, _info in _map.items():
        model_cls = serializer_cls.Meta.model
        qset = model_cls.objects.filter(
            id__in=_info["ids"], usrprof=profile, visible=True
        )

        class fake_request:
            query_params = {"fields": ",".join(_info["fields"])}

        extra_context = {"request": fake_request}
        serializer = serializer_cls(many=True, instance=qset, context=extra_context)
        out[_info["output_key"]] = serializer.data
    return out
