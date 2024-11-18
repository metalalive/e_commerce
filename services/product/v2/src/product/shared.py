import asyncio
from types import ModuleType

from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)
from product.adapter.repository import AbstractTagRepo


class AppDataStore:
    def __init__(self, tag_repo: AbstractTagRepo):
        self._tag_repo = tag_repo

    @staticmethod
    async def init(setting: ModuleType):
        cls_path = setting.DATABASES["tag"]["classpath"]
        tag_repo_cls = import_module_string(cls_path)
        db_credentials = get_credential_from_secrets(
            base_path=setting.SYS_BASE_PATH,
            secret_path=setting.SECRETS_FILE_PATH,
            secret_map={"cfdntl": setting.DATABASES["confidential_path"]},
        )
        setting.DATABASES["tag"]["cfdntl"] = db_credentials["cfdntl"]
        loop = asyncio.get_running_loop()
        tag_repo = await tag_repo_cls.init(setting.DATABASES["tag"], loop=loop)
        return AppDataStore(tag_repo=tag_repo)

    async def deinit(self):
        await self._tag_repo.deinit()

    @property
    def tag(self) -> AbstractTagRepo:
        return self._tag_repo


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
