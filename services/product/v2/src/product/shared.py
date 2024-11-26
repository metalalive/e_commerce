import asyncio
from types import ModuleType

from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)
from product.adapter.repository import AbstractTagRepo, AbstractAttrLabelRepo


class AppDataStore:
    def __init__(self, tag_repo: AbstractTagRepo, attr_repo: AbstractAttrLabelRepo):
        self._tag_repo = tag_repo
        self._attr_repo = attr_repo

    @staticmethod
    async def init(setting: ModuleType):
        db_credentials = get_credential_from_secrets(
            base_path=setting.SYS_BASE_PATH,
            secret_path=setting.SECRETS_FILE_PATH,
            secret_map={"cfdntl": setting.DATABASES["confidential_path"]},
        )
        setting.DATABASES["tag"]["cfdntl"] = db_credentials["cfdntl"]
        loop = asyncio.get_running_loop()

        cls_path = setting.DATABASES["tag"]["classpath"]
        tag_repo_cls = import_module_string(cls_path)
        tag_repo = await tag_repo_cls.init(setting.DATABASES["tag"], loop=loop)
        cls_path = setting.DATABASES["attribute-label"]["classpath"]
        attr_repo_cls = import_module_string(cls_path)
        attr_repo = await attr_repo_cls.init(
            setting.DATABASES["attribute-label"], loop=loop
        )
        return AppDataStore(tag_repo=tag_repo, attr_repo=attr_repo)

    async def deinit(self):
        await self._tag_repo.deinit()
        await self._attr_repo.deinit()

    @property
    def tag(self) -> AbstractTagRepo:
        return self._tag_repo

    @property
    def prod_attri(self) -> AbstractAttrLabelRepo:
        return self._attr_repo


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
