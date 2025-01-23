import logging
import os
import asyncio
from importlib import import_module
from typing import Dict, List

from celery.backends.rpc import RPCBackend as CeleryRpcBackend

from ecommerce_common.util import import_module_string
from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

from product.adapter.repository import AbstractSaleItemRepo
from product.model import SaleableItemModel

_logger = logging.getLogger(__name__)
evtloop = asyncio.new_event_loop()

cfg_mod_path = os.getenv("CELERY_CONFIG_MODULE", "settings.common")
_settings = import_module(cfg_mod_path)
shr_ctx_cls = import_module_string(dotted_path=_settings.SHARED_CONTEXT)
shr_ctx = evtloop.run_until_complete(shr_ctx_cls.init(setting=_settings))


@celery_app.task(
    backend=CeleryRpcBackend(app=celery_app),
    queue="rpc_productmgt_get_product",
    routing_key="rpc.product.get_product",
)
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def get_product(item_ids: List[int], profile: int) -> Dict:
    routine = get_saleitems_data(
        item_ids, profile, repo=shr_ctx.datastore.saleable_item
    )
    result = evtloop.run_until_complete(routine)
    return {"result": result}


async def get_saleitems_data(
    item_ids: List[int],
    profile: int,
    repo: AbstractSaleItemRepo,
) -> List[Dict]:
    item_ids = list(set(item_ids))
    ms: List[SaleableItemModel] = await repo.fetch_many(
        ids=item_ids, usrprof=profile, visible_only=True
    )
    data = [m.to_dto().model_dump() for m in ms]
    discard_fields = ["usr_prof", "name", "visible", "tags", "media_set"]
    # reserved fields: id_ , attributes, last_update
    for d in data:
        for fname in discard_fields:
            d.pop(fname)
    return data
