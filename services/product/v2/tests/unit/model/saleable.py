import pytest

from product.api.dto import AttrDataTypeDto, SaleItemAttriReqDto, SaleItemCreateReqDto
from product.model import (
    AttrLabelModel,
    AttriLabelError,
    AttriLabelErrorReason,
    SaleItemAttriModel,
    SaleableItemModel,
    TagModel,
)


class TestAttributeValue:
    def test_create_ok(self):
        labels = [
            AttrLabelModel(id_="x1", name="Age", dtype=AttrDataTypeDto.Integer),
            AttrLabelModel(id_="x2", name="Name", dtype=AttrDataTypeDto.String),
            AttrLabelModel(id_="x3", name="IsQualified", dtype=AttrDataTypeDto.Boolean),
        ]
        reqs = [
            SaleItemAttriReqDto(id_="x1", value=55),
            SaleItemAttriReqDto(id_="x2", value="Red Head"),
            SaleItemAttriReqDto(id_="x3", value=True),
        ]
        result = SaleItemAttriModel.from_req(labels, reqs)
        assert len(result) == 3
        assert result[0].label.id_ == "x1"
        assert result[0].value == 55
        assert result[1].label.id_ == "x2"
        assert result[1].value == "Red Head"
        assert result[2].label.id_ == "x3"
        assert result[2].value is True

    def test_create_missing_ids(self):
        labels = [
            AttrLabelModel(id_="g1", name="Size", dtype=AttrDataTypeDto.String),
            AttrLabelModel(id_="g10", name="Color", dtype=AttrDataTypeDto.String),
        ]
        reqs = [
            SaleItemAttriReqDto(id_="g1", value="Large"),
            SaleItemAttriReqDto(id_="g2", value="Red"),
            SaleItemAttriReqDto(id_="g3", value=1819),
        ]
        with pytest.raises(AttriLabelError) as e:
            SaleItemAttriModel.from_req(labels, reqs)
        assert e.value.reason == AttriLabelErrorReason.MissingID
        assert set(e.value.detail["nonexist-attribute-labels"]) == set(["g3", "g2"])

    def test_create_dtype_error(self):
        labels = [
            AttrLabelModel(id_="x3", name="IsQualified", dtype=AttrDataTypeDto.Boolean),
            AttrLabelModel(id_="ee1", name="Size", dtype=AttrDataTypeDto.Integer),
        ]
        reqs = [
            SaleItemAttriReqDto(id_="ee1", value="Large"),
            SaleItemAttriReqDto(id_="x3", value=False),
        ]
        with pytest.raises(AttriLabelError) as e:
            SaleItemAttriModel.from_req(labels, reqs)
        assert e.value.reason == AttriLabelErrorReason.InvalidData
        assert len(e.value.detail) == 1
        err_detail = e.value.detail[0]
        assert err_detail["id"] == "ee1"
        assert err_detail["expect_dtype"] is AttrDataTypeDto.Integer
        assert err_detail["received_value"] == "Large"


class TestSaleableItem:
    def test_create_from_req_ok(self):
        req = SaleItemCreateReqDto(
            name="Sample Item",
            visible=True,
            media_set=["resource-id-video", "resource-id-image"],
            tags=["scooter-1", "bike-2"],
            attributes=[SaleItemAttriReqDto(id_="xuy1", value="violet")],
        )
        tag_ms_map = {
            "scooter": [TagModel(_id=1, _label="Electronics")],
            "bike": [TagModel(_id=2, _label="BrandX")],
        }
        attri_val_ms = [
            SaleItemAttriModel(
                label=AttrLabelModel(
                    id_="xuy1", name="Color", dtype=AttrDataTypeDto.String
                ),
                value="violet",
            )
        ]
        usr_prof = 12345
        id_ = 67890

        item1 = SaleableItemModel.from_req(req, tag_ms_map, attri_val_ms, usr_prof)
        item2 = SaleableItemModel.from_req(
            req, tag_ms_map, attri_val_ms, usr_prof, id_=id_
        )

        assert item1.id_ > 0
        assert item2.id_ == id_
        assert item1.usr_prof == usr_prof
        assert item1.name == req.name
        assert item1.visible == req.visible
        assert item1.tags == tag_ms_map
        assert item1.attributes == attri_val_ms
        assert item1.media_set == req.media_set

        dto2 = item2.to_dto()
        assert dto2.id_ == id_
