from dataclasses import asdict
import pytest
from product.model import TagModel, TagErrorModel, TagErrorReason
from product.api.dto import TagCreateReqDto


class TestCreate:
    def test_dto_convert_ok(self):
        mock_parent_id = 9246
        mock_req = TagCreateReqDto(name="tag123", parent=mock_parent_id)
        tag_m = TagModel.from_req(mock_req)
        fieldmap = asdict(tag_m)
        assert fieldmap["_label"] == "tag123"
        assert fieldmap["_id"] == 0
        mock_resp = tag_m.to_resp(mock_req.parent)
        assert mock_resp.parent == mock_parent_id
        assert mock_resp.node.name == "tag123"
        assert mock_resp.node.id_ == 0

    def test_update_unknown_tree(self):
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m0 = TagModel.from_req(mock_req)
        mock_req = TagCreateReqDto(name="tag125", parent=None)
        tag_m1 = TagModel.from_req(mock_req)
        mock_tree = [tag_m0]
        with pytest.raises(TagErrorModel) as e:
            tag_m1.try_update(mock_tree, req_parent_id=None)
        assert e.value.reason == TagErrorReason.UnknownTree

    def test_update_missing_tree(self):
        mock_parent_id = 9246
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m = TagModel.from_req(mock_req)
        with pytest.raises(TagErrorModel) as e:
            tag_m.try_update(tree=[], req_parent_id=mock_parent_id)
        assert e.value.reason == TagErrorReason.MissingTree

    def test_update_missing_parent(self):
        mock_parent_id = pow(2, 32) - 1
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m0 = TagModel.from_req(mock_req)
        mock_req = TagCreateReqDto(name="tag125", parent=None)
        tag_m1 = TagModel.from_req(mock_req)
        mock_tree = [tag_m0]
        with pytest.raises(TagErrorModel) as e:
            tag_m1.try_update(mock_tree, req_parent_id=mock_parent_id)
        assert e.value.reason == TagErrorReason.MissingParent

    def test_update_fields_ok(self):
        # import pdb
        # pdb.set_trace()
        pass
