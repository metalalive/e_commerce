from enum import Enum, auto
from typing import Dict, List, Self, Optional
from dataclasses import dataclass
from asyncio.events import AbstractEventLoop

from product.model import TagTreeModel, AttrLabelModel, SaleableItemModel


class AppRepoFnLabel(Enum):
    TagSaveTree = auto()
    TagFetchTree = auto()
    TagDeleteTree = auto()
    TagNewTreeID = auto()
    AttrLabelCreate = auto()
    AttrLabelUpdate = auto()
    AttrLabelDelete = auto()
    AttrLabelSearch = auto()
    AttrLabelFetchByID = auto()
    SaleItemCreate = auto()
    SaleItemDelete = auto()
    SaleItemArchiveUpdate = auto()
    SaleItemFetchModel = auto()
    SaleItemGetMaintainer = auto()
    SaleItemNumCreated = auto()
    SaleItemSearch = auto()


@dataclass
class AppRepoError(Exception):
    fn_label: AppRepoFnLabel
    reason: Dict


class AbstractTagRepo:
    async def init(setting: Dict, loop: AbstractEventLoop):
        raise NotImplementedError("AbstractTagRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractTagRepo.deinit")

    async def fetch_tree(self, t_id: str) -> TagTreeModel:
        raise NotImplementedError("AbstractTagRepo.fetch_tree")

    async def save_tree(self, tree: TagTreeModel):
        raise NotImplementedError("AbstractTagRepo.save_tree")

    async def delete_tree(self, tree: TagTreeModel):
        raise NotImplementedError("AbstractTagRepo.delete_tree")

    async def new_tree_id(self) -> str:
        raise NotImplementedError("AbstractTagRepo.new_tree_id")


class AbstractAttrLabelRepo:
    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        raise NotImplementedError("AbstractAttrLabelRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractAttrLabelRepo.deinit")

    async def create(self, ms: List[AttrLabelModel]):
        raise NotImplementedError("AbstractAttrLabelRepo.create")

    async def update(self, ms: List[AttrLabelModel]):
        raise NotImplementedError("AbstractAttrLabelRepo.update")

    async def delete(self, ids: List[str]):
        raise NotImplementedError("AbstractAttrLabelRepo.delete")

    async def search(self, keyword: str) -> List[AttrLabelModel]:
        raise NotImplementedError("AbstractAttrLabelRepo.search")

    async def fetch_by_ids(self, ids: List[str]) -> List[AttrLabelModel]:
        raise NotImplementedError("AbstractAttrLabelRepo.fetch_by_ids")


class AbstractSaleItemRepo:
    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        raise NotImplementedError("AbstractSaleItemRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractSaleItemRepo.deinit")

    async def create(self, item_m: SaleableItemModel):
        raise NotImplementedError("AbstractSaleItemRepo.create")

    async def archive_and_update(self, item_m: SaleableItemModel):
        raise NotImplementedError("AbstractSaleItemRepo.archive_and_update")

    async def delete(self, id_: int):
        raise NotImplementedError("AbstractSaleItemRepo.delete")

    async def fetch(
        self, id_: int, visible_only: Optional[bool] = None
    ) -> SaleableItemModel:
        raise NotImplementedError("AbstractSaleItemRepo.fetch")

    async def get_maintainer(self, id_: int) -> int:
        raise NotImplementedError("AbstractSaleItemRepo.get_maintainer")

    async def num_items_created(self, usr_id: int) -> int:
        raise NotImplementedError("AbstractSaleItemRepo.num_items_created")

    async def search(
        self,
        keywords: List[str],
        visible_only: Optional[bool] = None,
        usr_id: Optional[int] = None,
    ) -> List[SaleableItemModel]:
        raise NotImplementedError("AbstractSaleItemRepo.search")
