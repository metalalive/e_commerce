import asyncio
import pytest
import pytest_asyncio

from ecommerce_common.util import (
    import_module_string,
    get_credential_from_secrets,
)

from product.model import TagModel, TagTreeModel
from product.api.dto import TagCreateReqDto
from product.adapter.repository import AppRepoError, AppRepoFnLabel


@pytest_asyncio.fixture(scope="module", loop_scope="module")
async def es_repo_tag(app_setting, es_mapping_init):
    db_credentials = get_credential_from_secrets(
        base_path=app_setting.SYS_BASE_PATH,
        secret_path=app_setting.SECRETS_FILE_PATH,
        secret_map={"cfdntl": app_setting.DATABASES["confidential_path"]},
    )
    app_setting.DATABASES["tag"]["cfdntl"] = db_credentials["cfdntl"]
    cls_path = app_setting.DATABASES["tag"]["classpath"]
    tag_repo_cls = import_module_string(cls_path)
    loop = asyncio.get_running_loop()
    # import pdb
    # pdb.set_trace()
    tag_repo = await tag_repo_cls.init(app_setting.DATABASES["tag"], loop=loop)
    yield tag_repo
    await tag_repo.deinit()


class TestSave:
    @staticmethod
    def setup_new_node(name):
        mock_req = TagCreateReqDto(name=name, parent=None)
        return TagModel.from_req(mock_req)

    @pytest.mark.asyncio(loop_scope="module")
    async def test_ok_1(self, es_repo_tag):
        cls = type(self)
        mock_tree_id = "beijing64Kill"
        mock_tree = TagTreeModel(_id=mock_tree_id)
        expect_labels = ["alpha", "beta", "gamma"]
        new_nodes = list(map(cls.setup_new_node, expect_labels))
        mock_tree.try_insert(new_nodes[0], parent_node_id=None)
        await es_repo_tag.save_tree(mock_tree)
        loaded_tree = await es_repo_tag.fetch_tree(mock_tree_id)
        assert loaded_tree.nodes[0]._label == "alpha"
        mock_tree.try_insert(new_nodes[1], parent_node_id=new_nodes[0]._id)
        mock_tree.try_insert(new_nodes[2], parent_node_id=new_nodes[0]._id)
        await es_repo_tag.save_tree(mock_tree)
        loaded_tree = await es_repo_tag.fetch_tree(mock_tree_id)
        actual_labels = [m._label for m in loaded_tree.nodes]
        assert set(actual_labels) == set(expect_labels)

    @pytest.mark.asyncio(loop_scope="module")
    async def test_ok_2(self, es_repo_tag):
        cls = type(self)
        mock_tree_id = "TienAnMenExecRiot"
        mock_tree = TagTreeModel(_id=mock_tree_id)
        expect_labels = ["delta", "epsilon", "theta", "pi"]
        new_nodes = list(map(cls.setup_new_node, expect_labels))
        mock_tree.try_insert(new_nodes[0], parent_node_id=None)
        mock_tree.try_insert(new_nodes[1], parent_node_id=new_nodes[0]._id)
        mock_tree.try_insert(new_nodes[2], parent_node_id=new_nodes[1]._id)
        mock_tree.try_insert(new_nodes[3], parent_node_id=new_nodes[1]._id)
        await es_repo_tag.save_tree(mock_tree)
        loaded_tree = await es_repo_tag.fetch_tree(mock_tree_id)
        actual_labels = [m._label for m in loaded_tree.nodes]
        assert set(actual_labels) == set(expect_labels)

    @pytest.mark.asyncio(loop_scope="module")
    async def test_empty(self, es_repo_tag):
        mock_tree = TagTreeModel(_id="e989")
        with pytest.raises(AppRepoError) as e:
            await es_repo_tag.save_tree(mock_tree)
        e = e.value
        assert e.fn_label == AppRepoFnLabel.TagSaveTree
        assert e.reason["num_nodes"] == 0

    @pytest.mark.asyncio(loop_scope="module")
    async def test_generate_new_id_ok(self, es_repo_tag):
        cls = type(self)
        expect_labels = ["mill", "hall", "hell"]
        expect_data = []
        for label in expect_labels:
            curr_tree_id = await es_repo_tag.new_tree_id()
            tree = TagTreeModel(_id=curr_tree_id)
            root_node = cls.setup_new_node(label)
            tree.try_insert(root_node, parent_node_id=None)
            await es_repo_tag.save_tree(tree)
            expect_data.append((curr_tree_id, label))
        assert len(expect_data) == 3
        for t_id, expect_label in expect_data:
            loaded_tree = await es_repo_tag.fetch_tree(t_id)
            assert len(loaded_tree.nodes) == 1
            assert loaded_tree.nodes[0]._label == expect_label


class TestFetch:
    @pytest.mark.asyncio(loop_scope="module")
    async def test_not_exist(self, es_repo_tag):
        with pytest.raises(AppRepoError) as e:
            await es_repo_tag.fetch_tree(t_id="route66")
        e = e.value
        assert e.fn_label == AppRepoFnLabel.TagFetchTree
        assert not e.reason["found"]


class TestDelete:
    @pytest.mark.asyncio(loop_scope="module")
    async def test_ok(self, es_repo_tag):
        mock_tree_id = "ch1na0rganHarve5t"
        mock_tree = TagTreeModel(_id=mock_tree_id)
        expect_labels = ["fwiw", "afaik", "iiuc"]
        new_nodes = list(map(TestSave.setup_new_node, expect_labels))
        mock_tree.try_insert(new_nodes[0], parent_node_id=None)
        mock_tree.try_insert(new_nodes[1], parent_node_id=new_nodes[0]._id)
        mock_tree.try_insert(new_nodes[2], parent_node_id=new_nodes[0]._id)
        await es_repo_tag.save_tree(mock_tree)
        loaded_tree = await es_repo_tag.fetch_tree(mock_tree_id)
        actual_labels = [a._label for a in loaded_tree.nodes]
        assert set(expect_labels) == set(actual_labels)
        await es_repo_tag.delete_tree(mock_tree)
        with pytest.raises(AppRepoError) as e:
            loaded_tree = await es_repo_tag.fetch_tree(mock_tree_id)
        e = e.value
        assert e.fn_label == AppRepoFnLabel.TagFetchTree
