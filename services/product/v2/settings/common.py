import os
from collections import OrderedDict
from pathlib import Path

APP_BASE_PATH = Path(__file__).resolve(strict=True).parent.parent
SYS_BASE_PATH = APP_BASE_PATH.parent.parent

if not os.environ.get("SYS_BASE_PATH"):
    os.environ["SYS_BASE_PATH"] = str(SYS_BASE_PATH)

SECRETS_FILE_PATH = "common/data/secrets.json"

ROUTER = "product.api.web.router"
SHARED_CONTEXT = "product.shared.SharedContext"
MIDDLEWARES = OrderedDict(
    [
        (
            "product.adapter.middleware.RateLimiter",
            {"max_reqs": 100, "interval_secs": 3},
        ),
        (
            "product.adapter.middleware.ReqBodySizeLimiter",
            {"max_nbytes": 2097152},
        ),
    ]
)

REPO_PKG_BASE = "product.adapter.repository"

DATABASES = {
    "tag": {
        "classpath": REPO_PKG_BASE + ".elasticsearch.ElasticSearchTagRepo",
        "ssl_enable": False,
        "timeout_secs": 16,
        "num_conns": 5,
        "db_name": "product-tags",
        "tree_id_length": 5,
    },
    "confidential_path": None,
}
