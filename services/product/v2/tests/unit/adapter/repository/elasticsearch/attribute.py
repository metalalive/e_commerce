import asyncio
from typing import Tuple, List
import pytest
import pytest_asyncio

from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)

from product.model import AttrLabelModel
from product.api.dto import (
    AttrCreateReqDto,
    AttrUpdateReqDto,
    AttrDataTypeDto,
)
from product.adapter.repository import (
    AbstractAttrLabelRepo,
    AppRepoError,
    AppRepoFnLabel,
)


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def es_repo_attri(app_setting, es_mapping_init):
    db_setup = app_setting.DATABASES["attribute-label"]
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


def setup_create_req(d: Tuple[str, AttrDataTypeDto]) -> AttrCreateReqDto:
    return AttrCreateReqDto(name=d[0], dtype=d[1].value)


def setup_update_req(d: Tuple[str, str, AttrDataTypeDto]) -> AttrUpdateReqDto:
    return AttrUpdateReqDto(id_=d[0], name=d[1], dtype=d[2].value)


class TestCreate:
    @staticmethod
    async def setup_create_many(
        repo: AbstractAttrLabelRepo,
        data: List[Tuple[str, AttrDataTypeDto]],
    ) -> List[AttrLabelModel]:
        reqs_d = list(map(setup_create_req, data))
        ms = AttrLabelModel.from_create_reqs(reqs_d)
        await repo.create(ms)
        return ms

    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, es_repo_attri):
        cls = type(self)
        mockdata = [
            ("dirt waterproof support", AttrDataTypeDto.Boolean),
            ("plain route66 width 57 arRon", AttrDataTypeDto.UnsignedInteger),
        ]
        ms = await cls.setup_create_many(es_repo_attri, mockdata)
        await asyncio.sleep(1)  # wait for ElasticSearch refresh documents
        result = await es_repo_attri.search(keyword="width 57")
        assert len(result) == 1
        assert result[0].name == "plain route66 width 57 arRon"
        assert result[0].id_ == ms[1].id_
        result = await es_repo_attri.search(keyword="waterprOOf")
        assert len(result) == 1
        assert result[0].name == "dirt waterproof support"
        for k in ["water proof", "waterpro"]:
            result = await es_repo_attri.search(keyword=k)
            assert len(result) == 0

    @pytest.mark.asyncio(loop_scope="session")
    async def test_resolve_duplicate(self, es_repo_attri):
        cls = type(self)
        mockdata = [("bottom width", AttrDataTypeDto.Integer)]
        ms = await cls.setup_create_many(es_repo_attri, mockdata)

        mockdata = ("foundation material", AttrDataTypeDto.String)
        req_d0 = setup_create_req(mockdata)
        orig_attr_m_id = ms[0].id_
        mockdata = (orig_attr_m_id, "bottom height", AttrDataTypeDto.UnsignedInteger)
        req_d1 = setup_update_req(mockdata)
        ms = AttrLabelModel.from_create_reqs([req_d0])
        ms.extend(AttrLabelModel.from_update_reqs([req_d1]))
        assert len(ms) == 2
        await es_repo_attri.create(ms)
        await asyncio.sleep(1)  # wait for ElasticSearch refresh documents
        result = await es_repo_attri.search(keyword="bottom")
        assert len(result) == 2

        def find_bottom_height(m) -> bool:
            return m.name == "bottom height"

        target = next(filter(find_bottom_height, result))
        assert target.id_ != orig_attr_m_id
        expect_readdata = [
            ("bottom width", AttrDataTypeDto.Integer),
            ("bottom height", AttrDataTypeDto.UnsignedInteger),
        ]
        actual_readdata = [(r.name, r.dtype) for r in result]
        assert set(expect_readdata) == set(actual_readdata)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_empty_input(self, es_repo_attri):
        with pytest.raises(AppRepoError) as e:
            await es_repo_attri.create([])
        e = e.value
        assert e.fn_label == AppRepoFnLabel.AttrLabelCreate
        assert e.reason["detail"] == "input-empty"
        with pytest.raises(AppRepoError) as e:
            await es_repo_attri.search("")
        e = e.value
        assert e.fn_label == AppRepoFnLabel.AttrLabelSearch
        assert e.reason["detail"] == "input-empty"


class TestUpdate:
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, es_repo_attri):
        mockdata = [
            ("auto brake mode", AttrDataTypeDto.Boolean),
            ("pedal cleat", AttrDataTypeDto.Integer),
        ]
        ms = await TestCreate.setup_create_many(es_repo_attri, mockdata)
        ms[0].name = "brake clay plate"
        ms[0].dtype = AttrDataTypeDto.String
        ms[1].name = "pedal chain material"
        await es_repo_attri.update(ms)
        await asyncio.sleep(1)  # wait for ElasticSearch refresh documents
        result = await es_repo_attri.search(keyword="pedal chain")
        assert len(result) == 1
        assert result[0].name == "pedal chain material"
        assert result[0].dtype == AttrDataTypeDto.Integer
        result = await es_repo_attri.search(keyword="clay plate")
        assert len(result) == 1
        assert result[0].name == "brake clay plate"
        assert result[0].dtype == AttrDataTypeDto.String


class TestFetch:
    @pytest.mark.asyncio(loop_scope="session")
    async def test_by_ids_ok(self, es_repo_attri):
        mockdata = [
            ("amplifier distortion effect", AttrDataTypeDto.String),
            ("amplifier decibels (dB)", AttrDataTypeDto.UnsignedInteger),
            ("I/O impedance (ohm)", AttrDataTypeDto.Integer),
            ("amplifier frequency response", AttrDataTypeDto.UnsignedInteger),
            ("device power-on voltage", AttrDataTypeDto.Integer),
        ]
        ms = await TestCreate.setup_create_many(es_repo_attri, mockdata)
        assert ms[1].name == "amplifier decibels (dB)"
        assert ms[3].name == "amplifier frequency response"
        ids = [ms[1].id_, ms[3].id_]
        readback = await es_repo_attri.fetch_by_ids(ids)
        expect = [(m.dtype, m.name) for m in ms if m.id_ in ids]
        actual = [(m.dtype, m.name) for m in readback]
        assert set(expect) == set(actual)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_by_ids_empty_err(self, es_repo_attri):
        with pytest.raises(AppRepoError) as e:
            _ = await es_repo_attri.fetch_by_ids(ids=[])
        e = e.value
        assert e.fn_label == AppRepoFnLabel.AttrLabelFetchByID
        assert e.reason["detail"] == "input-empty"


class TestDelete:
    @pytest.mark.asyncio(loop_scope="session")
    async def test_ok(self, es_repo_attri):
        mockdata = [
            ("pelindrone 1st", AttrDataTypeDto.Boolean),
            ("pelindrone 2nd", AttrDataTypeDto.Integer),
            ("pelindrone 3rd", AttrDataTypeDto.UnsignedInteger),
            ("pelindrone 4th", AttrDataTypeDto.String),
        ]
        ms = await TestCreate.setup_create_many(es_repo_attri, mockdata)
        await asyncio.sleep(1)  # wait for ElasticSearch refresh documents
        result = await es_repo_attri.search(keyword="pelindrone")
        expect_readdata = mockdata
        actual_readdata = [(r.name, r.dtype) for r in result]
        assert set(expect_readdata) == set(actual_readdata)

        remove_ids = [ms[0].id_, ms[2].id_]
        await es_repo_attri.delete(remove_ids)
        await asyncio.sleep(1)  # wait for ElasticSearch refresh documents
        result = await es_repo_attri.search(keyword="pelindrone")
        expect_readdata = [
            d for d in mockdata if d[0] in ["pelindrone 2nd", "pelindrone 4th"]
        ]
        actual_readdata = [(r.name, r.dtype) for r in result]
        assert set(expect_readdata) == set(actual_readdata)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_empty_input(self, es_repo_attri):
        with pytest.raises(AppRepoError) as e:
            await es_repo_attri.delete([])
        e = e.value
        assert e.fn_label == AppRepoFnLabel.AttrLabelDelete
        assert e.reason["detail"] == "input-empty"
