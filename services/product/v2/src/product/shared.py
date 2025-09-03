import asyncio
import logging
from types import ModuleType
from typing import Dict, Tuple, List

from guardpost.jwks import KeysProvider, JWKS, JWK as GuardPostJWK
from jwt.api_jwk import PyJWK
from jwt.utils import base64url_encode

from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)

from .adapter.repository import (
    AbstractTagRepo,
    AbstractSaleItemRepo,
    AbstractAttrLabelRepo,
)

_logger = logging.getLogger(__name__)


class AppDataStore:
    def __init__(self, repo_map: Dict):
        self._repo_map = repo_map

    @staticmethod
    async def init(setting: ModuleType):
        db_credentials = get_credential_from_secrets(
            base_path=setting.SYS_BASE_PATH,
            secret_path=setting.SECRETS_FILE_PATH,
            secret_map={"cfdntl": setting.DATABASES["confidential_path"]},
        )
        loop = asyncio.get_running_loop()

        async def init_one_repo(k, v) -> Tuple:
            v["cfdntl"] = db_credentials["cfdntl"]
            repo_cls = import_module_string(v["classpath"])
            repo = await repo_cls.init(v, loop=loop)
            return (k, repo)

        repo_kv_pairs = [
            await init_one_repo(k, v) for k, v in setting.DATABASES.items() if isinstance(v, Dict)
        ]
        return AppDataStore(repo_map=dict(repo_kv_pairs))

    async def deinit(self):
        _ = [await r.deinit() for r in self._repo_map.values()]

    @property
    def tag(self) -> AbstractTagRepo:
        return self._repo_map["tag"]

    @property
    def prod_attri(self) -> AbstractAttrLabelRepo:
        return self._repo_map["attribute-label"]

    @property
    def saleable_item(self) -> AbstractSaleItemRepo:
        return self._repo_map["saleable-item"]


class SharedContext:
    def __init__(self, setting: ModuleType, dstore: AppDataStore):
        self._setting = setting
        self._dstore = dstore

    @staticmethod
    async def init(setting: ModuleType):
        dstore = await AppDataStore.init(setting)
        return SharedContext(setting, dstore=dstore)

    async def deinit(self):
        await self._dstore.deinit()

    @property
    def datastore(self) -> AppDataStore:
        return self._dstore


class ExtendedKeysProvider(KeysProvider):
    def __init__(self, ks_setting: Dict):
        self._kstore = create_keystore_helper(cfg=ks_setting, import_fn=import_module_string)

    async def get_keys(self) -> JWKS:
        keyset: List[PyJWK] = self._kstore.all_pubkeys()
        kids = [k.key_id for k in keyset]
        _logger.debug("keyset length: %d, ids: %s", len(keyset), str(kids))
        _jwks = list(map(to_guardpost_jwk, keyset))
        return JWKS(_jwks)


def to_guardpost_jwk(wrapper: PyJWK) -> GuardPostJWK:
    key_in = wrapper.key
    pubnum = key_in.public_numbers()

    def encode_with_base64url(x: int) -> str:
        nbytes = (x.bit_length() + 7) >> 3
        seq = x.to_bytes(nbytes, byteorder="big")
        return base64url_encode(seq).decode("utf-8")

    raw = {
        "kid": wrapper.key_id,
        "kty": wrapper.key_type,
        "alg": wrapper.algorithm_name,
        "use": "sig",
        "n": encode_with_base64url(pubnum.n),
        "e": encode_with_base64url(pubnum.e),
    }
    # _logger.debug("raw key: %s",  str(raw))
    return GuardPostJWK.from_dict(raw)
