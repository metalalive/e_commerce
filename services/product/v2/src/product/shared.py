import asyncio
from types import ModuleType
from typing import Dict, Tuple

from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)

from product.adapter.repository import AbstractTagRepo, AbstractAttrLabelRepo


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
            await init_one_repo(k, v)
            for k, v in setting.DATABASES.items()
            if isinstance(v, Dict)
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
