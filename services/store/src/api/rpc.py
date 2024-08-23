import logging
from typing import Dict, Union

from celery.backends.rpc import RPCBackend as CeleryRpcBackend
from sqlalchemy.orm import Session

from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

from ..dto import StoreProfileDto
from ..models import StoreProfile
from ..shared import init_shared_context

_logger = logging.getLogger(__name__)

_shr_ctx = init_shared_context()


@celery_app.task(backend=CeleryRpcBackend(app=celery_app))
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def get_shop_profile(req: Dict) -> Dict:
    try:
        sid = req["store_id"]
    except KeyError as e:
        log_args = ["reason", "missing-store-id"]
        _logger.warning(None, *log_args)
        return {"error": "missing-store-id"}
    with Session(bind=_shr_ctx["db_engine"]) as session:
        try:
            query = session.query(StoreProfile).filter(StoreProfile.id == sid)
            saved_obj = query.first()
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
