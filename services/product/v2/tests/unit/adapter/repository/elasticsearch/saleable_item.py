import asyncio
from typing import List, Dict, Tuple, Any, Optional

import pytest
import pytest_asyncio

from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)
from product.api.dto import AttrDataTypeDto, SaleItemCreateReqDto
from product.model import (
    TagModel,
    AttrLabelModel,
    SaleItemAttriModel,
    SaleableItemModel,
)

from product.adapter.repository import (
    AbstractSaleItemRepo,
    AppRepoError,
    AppRepoFnLabel,
)


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def es_repo_saleitem(app_setting, es_mapping_init):
    db_setup = app_setting.DATABASES["saleable-item"]
    db_credentials = get_credential_from_secrets(
        base_path=app_setting.SYS_BASE_PATH,
        secret_path=app_setting.SECRETS_FILE_PATH,
        secret_map={"cfdntl": app_setting.DATABASES["confidential_path"]},
    )
    db_setup["cfdntl"] = db_credentials["cfdntl"]
    repo_cls = import_module_string(db_setup["classpath"])
    loop = asyncio.get_running_loop()
    repo = await repo_cls.init(db_setup, loop=loop)
    yield repo
    await repo.deinit()


def verify_items_equlity(item1: SaleableItemModel, item2: SaleableItemModel):
    assert item1.id_ == item2.id_
    assert item1.name == item2.name
    assert item1.usr_prof == item2.usr_prof
    assert item1.visible == item2.visible
    assert set(item1.media_set) == set(item2.media_set)

    expect = build_verify_tag_data(item2.tags)
    actual = build_verify_tag_data(item1.tags)
    assert set(expect) == set(actual)
    expect = build_verify_attri_data(item2.attributes)
    actual = build_verify_attri_data(item1.attributes)
    assert set(expect) == set(actual)


def build_verify_tag_data(tagmap: Dict) -> List[Tuple[str, int, str]]:
    return [(k, v._id, v._label) for k, vs in tagmap.items() for v in vs]


def build_verify_attri_data(
    attributes: List[SaleItemAttriModel],
) -> List[Tuple[str, str, AttrDataTypeDto, Any]]:
    return [(a.label.id_, a.label.name, a.label.dtype, a.value) for a in attributes]


class TestCreate:
    @staticmethod
    def setup_model(
        usr_prof: int,
        req_data: SaleItemCreateReqDto,
        tag_data: Dict[str, List[Tuple[int, str]]],
        attr_data: List[Tuple[str, str, AttrDataTypeDto, Any]],
        id_: Optional[int] = None,
    ) -> SaleableItemModel:
        tag_ms_map = {
            k: [TagModel(_id=v[0], _label=v[1]) for v in vs]
            for k, vs in tag_data.items()
        }
        attri_val_ms = [
            SaleItemAttriModel(
                label=AttrLabelModel(id_=a[0], name=a[1], dtype=a[2]),
                value=a[3],
            )
            for a in attr_data
        ]
        return SaleableItemModel.from_req(
            req_data, tag_ms_map, attri_val_ms, usr_prof, id_=id_
        )

    @classmethod
    async def setup_create_one(
        cls,
        repo: AbstractSaleItemRepo,
        usr_prof: int,
        req_data: SaleItemCreateReqDto,
        tag_data: Dict[str, List[Tuple[int, str]]],
        attr_data: List[Tuple[str, str, AttrDataTypeDto, Any]],
        id_: Optional[int] = None,
    ) -> SaleableItemModel:
        saleitem_m = cls.setup_model(
            usr_prof=usr_prof,
            req_data=req_data,
            tag_data=tag_data,
            attr_data=attr_data,
            id_=id_,
        )
        await repo.create(saleitem_m)
        return saleitem_m

    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, es_repo_saleitem):
        cls = type(self)
        expect_usr_prof = 12345
        req_data = SaleItemCreateReqDto(
            name="Sample Item",
            visible=True,
            media_set=["resource-id-video", "resource-id-image"],
            tags=["scooter-1", "bike-2", "scooter-3", "bike-4"],
            attributes=[],
        )
        tag_data = {
            "scooter": [(1, "Electronics"), (3, "Microwave")],
            "bike": [(2, "Giant"), (4, "Melida")],
        }
        attr_data = [
            ("xuy1", "Color", AttrDataTypeDto.String, "violet"),
            ("8chi", "volt output", AttrDataTypeDto.Integer, -3),
            ("Hela", "volt in", AttrDataTypeDto.UnsignedInteger, 110),
        ]
        saleitem_m_created = await cls.setup_create_one(
            es_repo_saleitem,
            usr_prof=expect_usr_prof,
            req_data=req_data,
            tag_data=tag_data,
            attr_data=attr_data,
        )
        assert saleitem_m_created.id_ > 0
        assert saleitem_m_created.id_ < pow(2, 64)

        req_data = SaleItemCreateReqDto(
            name="Urloksua",
            visible=True,
            media_set=["resource-id-audio", "resource-id-video"],
            tags=["cooker-8", "sink-19", "cooker-18", "sink-56"],
            attributes=[],
        )
        tag_data = {
            "cooker": [(8, "stone pot"), (18, "debone knife")],
            "sink": [(56, "jaJaA"), (19, "volka drain")],
        }
        attr_data = [
            ("bLie", "halal-certified", AttrDataTypeDto.Boolean, True),
            ("Hela", "volt in", AttrDataTypeDto.UnsignedInteger, 13),
        ]
        another_item_created = await cls.setup_create_one(
            es_repo_saleitem,
            usr_prof=expect_usr_prof,
            req_data=req_data,
            tag_data=tag_data,
            attr_data=attr_data,
            id_=saleitem_m_created.id_,
        )
        assert another_item_created.id_ != saleitem_m_created.id_
        assert another_item_created.id_ > 0
        assert another_item_created.id_ < pow(2, 64)

        await asyncio.sleep(1)  # wait for ElasticSearch refresh documents

        readback = await es_repo_saleitem.fetch(saleitem_m_created.id_)
        verify_items_equlity(readback, saleitem_m_created)
        read_usr_prof = await es_repo_saleitem.get_maintainer(
            id_=saleitem_m_created.id_
        )
        assert read_usr_prof == expect_usr_prof


class TestUpdate:
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, es_repo_saleitem):
        req_data = SaleItemCreateReqDto(
            name="Kurshuamir Coat",
            visible=False,
            media_set=["resource-id-image-1", "resource-id-image-2"],
            tags=["xiug-1", "fj3e-2", "xiug-3", "fj3e-4"],
            attributes=[],
        )
        tag_data = {
            "xiug": [(1, "Label 1"), (3, "Label 3")],
            "fj3e": [(2, "Label 2"), (4, "Label 4")],
        }
        attr_data = [
            ("attr1id", "Material", AttrDataTypeDto.String, "Aluminum"),
            ("attr2id", "Weight", AttrDataTypeDto.Integer, 15),
            ("attr3id", "Durable", AttrDataTypeDto.Boolean, True),
            ("attr4id", "Warranty", AttrDataTypeDto.UnsignedInteger, 5),
        ]
        saleitem_m = await TestCreate.setup_create_one(
            es_repo_saleitem,
            usr_prof=12347,
            req_data=req_data,
            tag_data=tag_data,
            attr_data=attr_data,
        )
        new_tags = [
            TagModel(_id=15, _label="fireball"),
            TagModel(_id=16, _label="bang"),
        ]
        saleitem_m.tags["xiug"].extend(new_tags)
        saleitem_m.name = "Fabulous Coat"
        old_attr = next(
            filter(lambda a: a.label.id_ == "attr4id", saleitem_m.attributes)
        )
        old_attr.value = 99
        new_attr = SaleItemAttriModel(
            label=AttrLabelModel(
                id_="ymLo", name="max-celcius", dtype=AttrDataTypeDto.Integer
            ),
            value=-23,
        )
        saleitem_m.attributes.append(new_attr)
        await es_repo_saleitem.archive_and_update(saleitem_m)
        readback = await es_repo_saleitem.fetch(saleitem_m.id_)
        verify_items_equlity(readback, saleitem_m)


class TestDelete:
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, es_repo_saleitem):
        req_data = SaleItemCreateReqDto(
            name="ToDelete Item",
            visible=True,
            media_set=["resource-id-video", "resource-id-image"],
            tags=["delete-1", "delete-2"],
            attributes=[],
        )
        tag_data = {"delete": [(1, "Tag1"), (2, "Tag2")]}
        attr_data = [("attr1", "DeletableAttr", AttrDataTypeDto.String, "DeleteValue")]
        saleitem_m_created = await TestCreate.setup_create_one(
            es_repo_saleitem,
            usr_prof=12350,
            req_data=req_data,
            tag_data=tag_data,
            attr_data=attr_data,
        )
        readback = await es_repo_saleitem.fetch(saleitem_m_created.id_)
        verify_items_equlity(readback, saleitem_m_created)

        await es_repo_saleitem.delete(saleitem_m_created.id_)
        with pytest.raises(AppRepoError) as e:
            await es_repo_saleitem.fetch(saleitem_m_created.id_)
        e = e.value
        assert e.fn_label == AppRepoFnLabel.SaleItemFetchModel
        assert not e.reason["found"]
        assert int(e.reason["_id"]) == saleitem_m_created.id_
