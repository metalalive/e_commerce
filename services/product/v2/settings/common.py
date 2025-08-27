import os
from collections import OrderedDict
from pathlib import Path

APP_BASE_PATH = Path(__file__).resolve(strict=True).parent.parent
SYS_BASE_PATH = APP_BASE_PATH.parent.parent

if not os.environ.get("SYS_BASE_PATH"):
    os.environ["SYS_BASE_PATH"] = str(SYS_BASE_PATH)

SECRETS_FILE_PATH = "common/data/secrets.json"

USRMGT_SERVER_BASEADDR = os.environ["AUTH_SERVER_ADDR"]

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

EXCEPTION_HANDLING_FUNCTIONS = {
    "product.model.TagErrorModel": "product.api.web.tag.exception_handler"
}

REPO_PKG_BASE = "product.adapter.repository"

# TODO, improve the database address setup
DB_HOST_DOMAIN_NAME = os.environ["DB_ES_HOST"]
DB_HOST_PORT = int(os.environ["DB_ES_PORT"])

DATABASES = {
    "tag": {
        "classpath": REPO_PKG_BASE + ".elasticsearch.ElasticSearchTagRepo",
        "ssl_enable": False,
        "timeout_secs": 16,
        "num_conns": 5,
        "db_name": "product-tags",
        "db_addr": {"HOST": DB_HOST_DOMAIN_NAME, "PORT": DB_HOST_PORT},
        "tree_id_length": 5,
    },
    "attribute-label": {
        "classpath": REPO_PKG_BASE + ".elasticsearch.ElasticSearchAttrLabelRepo",
        "ssl_enable": False,
        "timeout_secs": 38,
        "num_conns": 6,
        "db_name": "product-attribute-labels",
        "db_addr": {"HOST": DB_HOST_DOMAIN_NAME, "PORT": DB_HOST_PORT},
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
        "db_addr": {"HOST": DB_HOST_DOMAIN_NAME, "PORT": DB_HOST_PORT},
    },
    "confidential_path": None,
}  # --- end of DATABASE clause

KEYSTORE = {
    "keystore": "ecommerce_common.auth.keystore.BaseAuthKeyStore",
    "persist_pubkey_handler": {
        "module_path": "ecommerce_common.auth.jwt.RemoteJWKSPersistHandler",
        "init_kwargs": {
            "url": f"{USRMGT_SERVER_BASEADDR}:8008/jwks",
            "name": "remote_pubkey",
            "lifespan_hrs": 13,
        },
    },
}

AUTH_KEY_PROVIDER = "product.shared.ExtendedKeysProvider"
JWT_ISSUER = f"{USRMGT_SERVER_BASEADDR}/login"
