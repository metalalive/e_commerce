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
    "attribute-label": {
        "classpath": REPO_PKG_BASE + ".elasticsearch.ElasticSearchAttrLabelRepo",
        "ssl_enable": False,
        "timeout_secs": 38,
        "num_conns": 6,
        "db_name": "product-attribute-labels",
    },
    "saleable-item": {
        "classpath": REPO_PKG_BASE + ".elasticsearch.ElasticSearchSaleItemRepo",
        "ssl_enable": False,
        "timeout_secs": 45,
        "num_conns": 7,
        "db_names": {
            "latest": "product-saleable-items-v0.0.1",
            "history": "product-saleable-items-snapshot-%s",
        },
    },
    "confidential_path": None,
}  # --- end of DATABASE clause

KEYSTORE = {
    "keystore": "ecommerce_common.auth.keystore.BaseAuthKeyStore",
    "persist_pubkey_handler": {
        "module_path": "ecommerce_common.auth.jwt.RemoteJWKSPersistHandler",
        "init_kwargs": {
            "url": "http://localhost:8008/jwks",
            "name": "remote_pubkey",
            "lifespan_hrs": 13,
        },
    },
}

AUTH_KEY_PROVIDER = "product.shared.ExtendedKeysProvider"
JWT_ISSUER = "http://localhost:8008/login"
