import os
from pathlib import Path
from importlib import import_module
from datetime import datetime, date, time, timedelta
import logging

from .celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

_logger = logging.getLogger(__name__)

srv_basepath = Path(os.environ["SERVICE_BASE_PATH"]).resolve(strict=True)


@celery_app.task
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def clean_old_log_localhost(max_days_keep=100):
    num_removed = 0
    log_path = srv_basepath.joinpath("tmp/log/dev")
    if log_path.exists():
        for curr_node in log_path.iterdir():
            stat = curr_node.stat()
            t0 = datetime.utcnow() - timedelta(days=max_days_keep)
            t1 = datetime.utcfromtimestamp(stat.st_mtime)
            if t0 > t1:
                if curr_node.is_file:
                    os.remove(curr_node)
                    num_removed += 1
    return num_removed


## TODO, for logging data, replace elasticsearch with one of the alternatives :
## OpenSearch, MeiliSerach, Typesense

