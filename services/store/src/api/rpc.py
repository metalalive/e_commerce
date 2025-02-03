import logging
import asyncio
from typing import Dict

from celery.backends.rpc import RPCBackend as CeleryRpcBackend
from sqlalchemy import select as sa_select
from sqlalchemy.orm import selectinload
from sqlalchemy.ext.asyncio import AsyncSession

from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

from ..dto import StoreProfileDto
from ..models import StoreProfile
from ..shared import app_shared_context_start

_logger = logging.getLogger(__name__)

evloop = asyncio.new_event_loop()

_shr_ctx = evloop.run_until_complete(app_shared_context_start(None))

_shr_ctx["evt_loop"] = evloop

# NOTE:
# Celery currently does not support async task-handling function,
# See the tracking issue --> https://github.com/celery/celery/issues/6552
#
# Current workaround in this service is to share the same event loop to
# all sessions that have been created in the async engine. Remind that
# every async session is bound to a specific event loop, if the loop does
# not match then the session immediately raises greenlet RuntimeError.


@celery_app.task(backend=CeleryRpcBackend(app=celery_app))
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def get_shop_profile(req: Dict) -> Dict:
    try:
        sid = req["store_id"]
        routine = _get_shop_profile(_shr_ctx["db_engine"], sid)
        # Note, don't use `asyncio.run(...)` , as it automatically closes the loop
        # after the given task routine is done.
        return _shr_ctx["evt_loop"].run_until_complete(routine)
    except KeyError as e:
        log_args = ["reason", "missing-store-id", "detail", str(e)]
        _logger.warning(None, *log_args)
        return {"error": "missing-store-id"}


async def _get_shop_profile(db_engine, sid: int) -> Dict:
    related_attrs = [
        StoreProfile.phones,
        StoreProfile.emails,
        StoreProfile.location,
        StoreProfile.open_days,
        StoreProfile.staff,
    ]
    related_cols = map(lambda v: selectinload(v), related_attrs)
    stmt = sa_select(StoreProfile).filter(StoreProfile.id == sid).options(*related_cols)
    async with AsyncSession(bind=db_engine) as session:
        try:
            resultset = await session.execute(stmt)
            row = resultset.first()
            saved_obj = row[0]
        except Exception as e:
            log_args = ["reason", str(e)]
            _logger.error(None, *log_args)
            saved_obj = None
        if saved_obj:
            store_d = StoreProfileDto.model_validate(saved_obj)
            out = store_d.model_dump()
        else:
            log_args = ["reason", "store-not-exist", "store_id", str(sid)]
            _logger.warning(None, *log_args)
            out = {"error": "store-not-exist"}
    return out
