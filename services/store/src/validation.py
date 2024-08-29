import logging
from datetime import datetime, time as py_time
from functools import partial
from typing import Optional, List, Dict, Union

from typing_extensions import Annotated
from pydantic import (
    BaseModel as PydanticBaseModel,
    RootModel as PydanticRootModel,
    PositiveInt,
    StringConstraints,
    EmailStr,
    field_validator,
    ValidationError,
    SkipValidation,
    ConfigDict,
)
from fastapi import HTTPException as FastApiHTTPException, status as FastApiHTTPstatus

from ecommerce_common.models.enums.base import AppCodeOptions, ActivationStatus
from ecommerce_common.models.contact.sqlalchemy import CountryCodeEnum
from .shared import shared_ctx
from .models import (
    EnumWeekDay,
    SaleableTypeEnum,
    StoreEmail,
    StorePhone,
    OutletLocation,
    StoreCurrency,
    StoreProfile,
    StoreStaff,
    HourOfOperation,
    StoreProductAvailable,
)
from .dto import (
    StoreEmailDto,
    StorePhoneDto,
    ShopLocationDto,
    StoreStaffDto,
    StoreDtoError,
    BusinessHoursDayDto,
)

_logger = logging.getLogger(__name__)


class StoreStaffReqBody(StoreStaffDto):
    def __init__(self, *args, **kwargs):
        try:
            super().__init__(*args, **kwargs)
        except StoreDtoError as e:
            raise FastApiHTTPException(
                detail=e.detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )


class BusinessHoursDayReqBody(BusinessHoursDayDto):
    def __init__(self, *args, **kwargs):
        try:
            super().__init__(*args, **kwargs)
        except StoreDtoError as e:
            raise FastApiHTTPException(
                detail=e.detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )


class NewStoreProfileReqBody(PydanticBaseModel):
    label: str
    supervisor_id: PositiveInt
    currency: StoreCurrency
    active: Optional[bool] = False
    emails: Optional[List[StoreEmailDto]] = []
    phones: Optional[List[StorePhoneDto]] = []
    location: Optional[ShopLocationDto] = None
    quota: SkipValidation[Optional[Dict]] = (
        None  # should be updated by `NewStoreProfilesReqBody`
    )

    def _pydantic_to_sqlalchemy(item):
        item = item.model_dump()  # convert to pure `dict` type
        item.pop("quota")
        item["emails"] = list(map(lambda d: StoreEmail(**d), item.get("emails", [])))
        item["phones"] = list(map(lambda d: StorePhone(**d), item.get("phones", [])))
        if item.get("location"):
            item["location"] = OutletLocation(**item["location"])
        obj = StoreProfile(**item)
        return obj


def _get_supervisor_auth(prof_ids):  # TODO, async operation
    reply_evt = shared_ctx["auth_app_rpc"].get_profile(
        ids=prof_ids, fields=["id", "auth", "quota"]
    )
    if not reply_evt.finished:
        for _ in range(
            shared_ctx["settings"].NUM_RETRY_RPC_RESPONSE
        ):  # TODO, async task
            reply_evt.refresh(retry=False, timeout=0.5, num_of_msgs_fetch=1)
            if reply_evt.finished:
                break
            else:
                pass
    rpc_response = reply_evt.result
    if rpc_response["status"] != reply_evt.status_opt.SUCCESS:
        raise FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_503_SERVICE_UNAVAILABLE,
            headers={},
            detail={"app_code": [AppCodeOptions.user_management.value[0]]},
        )
    return rpc_response["result"]


class NewStoreProfilesReqBody(PydanticRootModel[List[NewStoreProfileReqBody]]):
    @field_validator("root")  # map to default field name in the root-model
    def validate_list_items(cls, values):
        assert values and any(values), "Empty request body Not Allowed"
        req_prof_ids = list(set(map(lambda obj: obj.supervisor_id, values)))
        supervisor_verified = _get_supervisor_auth(req_prof_ids)
        quota_arrangement = cls._estimate_quota(values, supervisor_verified)
        cls._contact_common_quota_check(
            values, quota_arrangement, label="emails", mat_model_cls=StoreEmail
        )
        cls._contact_common_quota_check(
            values, quota_arrangement, label="phones", mat_model_cls=StorePhone
        )
        return values

    def _estimate_quota(values, supervisor_verified):
        supervisor_verified = {item["id"]: item for item in supervisor_verified}
        out = {}

        def _fn(item):
            err = _get_quota_arrangement_helper(
                supervisor_verified, req_prof_id=item.supervisor_id, out=out
            )
            if not any(err):
                item.quota = out[item.supervisor_id]
            return err

        err_content = list(map(_fn, values))
        if any(err_content):
            raise FastApiHTTPException(
                detail=err_content,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
        return out

    @classmethod
    def _contact_common_quota_check(
        cls,
        req,
        quota_arrangement: dict,
        label: str,
        mat_model_cls: Union[StoreEmail, StorePhone],
    ):
        def _inner_chk(item):
            err = {}
            num_new_items = len(getattr(item, label))
            max_limit = quota_arrangement[item.supervisor_id][mat_model_cls]
            if max_limit < num_new_items:
                err["supervisor_id"] = item.supervisor_id
                err[label] = {
                    "type": "limit-exceed",
                    "max_limit": max_limit,
                    "num_new_items": num_new_items,
                }
            return err

        err_content = list(map(_inner_chk, req))
        if any(err_content):
            raise FastApiHTTPException(
                detail=err_content,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
            )

    async def validate_quota(self, session):
        # quota check, for current user who adds these new items
        new_stores = list(
            map(NewStoreProfileReqBody._pydantic_to_sqlalchemy, self.root)
        )
        profile_ids = list(map(lambda obj: obj.supervisor_id, self.root))
        quota_arrangement = {obj.supervisor_id: obj.quota for obj in self.root}
        quota_chk_result = await StoreProfile.quota_stats(
            new_stores, session=session, target_ids=profile_ids
        )

        def _inner_chk(item):
            err = {}
            num_existing_items = quota_chk_result[item.supervisor_id][
                "num_existing_items"
            ]
            num_new_items = quota_chk_result[item.supervisor_id]["num_new_items"]
            curr_used = num_existing_items + num_new_items
            max_limit = quota_arrangement[item.supervisor_id][StoreProfile]
            if max_limit < curr_used:
                err["supervisor_id"] = item.supervisor_id
                err["store_profile"] = {
                    "type": "limit-exceed",
                    "max_limit": max_limit,
                    "num_new_items": num_new_items,
                    "num_existing_items": num_existing_items,
                }
            return err

        err_content = list(map(_inner_chk, self.root))
        if any(err_content):
            raise FastApiHTTPException(
                detail=err_content,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
            )
        return new_stores


## end of class NewStoreProfilesReqBody()


class ExistingStoreProfileReqBody(PydanticBaseModel):
    label: str
    active: bool
    currency: StoreCurrency
    emails: Optional[List[StoreEmailDto]] = []
    phones: Optional[List[StorePhoneDto]] = []
    location: Optional[ShopLocationDto] = None


class StoreSupervisorReqBody(PydanticBaseModel):
    supervisor_id: PositiveInt  # for new supervisor

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        req_prof_id = self.supervisor_id
        supervisor_verified = _get_supervisor_auth([req_prof_id])
        quota_arrangement = self._estimate_quota(supervisor_verified, req_prof_id)
        self.metadata = {"quota_arrangement": quota_arrangement}

    def __setattr__(self, name, value):
        if name == "metadata":
            # the attribute is for internal use, skip type checking
            self.__dict__[name] = value
        else:
            super().__setattr__(name, value)

    def _estimate_quota(self, supervisor_verified, req_prof_id):
        supervisor_verified = {item["id"]: item for item in supervisor_verified}
        out = {}
        err_detail = _get_quota_arrangement_helper(
            supervisor_verified, req_prof_id=req_prof_id, out=out
        )
        if any(err_detail):
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
        return out

    async def validate_quota(self, session):
        prof_id = self.supervisor_id
        quota_arrangement = self.metadata["quota_arrangement"]
        quota_chk_result = await StoreProfile.quota_stats(
            [], session=session, target_ids=[prof_id]
        )
        err = {}
        num_existing_items = quota_chk_result[prof_id]["num_existing_items"]
        num_new_items = 1
        curr_used = num_existing_items + num_new_items
        max_limit = quota_arrangement[prof_id][StoreProfile]
        if max_limit < curr_used:
            err["supervisor_id"] = prof_id
            err["store_profile"] = {
                "type": "limit-exceed",
                "max_limit": max_limit,
                "num_new_items": num_new_items,
                "num_existing_items": num_existing_items,
            }
        if any(err):
            raise FastApiHTTPException(
                detail=err, headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN
            )


class StoreStaffsReqBody(PydanticRootModel[List[StoreStaffReqBody]]):
    @field_validator("root")
    def validate_list_items(cls, values):
        staff_ids = set(map(lambda obj: obj.staff_id, values))
        if len(staff_ids) != len(values):
            err_detail = {"code": "duplicate", "field": ["staff_id"]}
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
        return values

    def validate_staff(self, supervisor_id: int):
        staff_ids = list(map(lambda obj: obj.staff_id, self.root))
        reply_evt = shared_ctx["auth_app_rpc"].profile_descendant_validity(
            asc=supervisor_id, descs=staff_ids
        )
        if not reply_evt.finished:
            for _ in range(
                shared_ctx["settings"].NUM_RETRY_RPC_RESPONSE
            ):  # TODO, async task
                reply_evt.refresh(retry=False, timeout=0.4, num_of_msgs_fetch=1)
                if reply_evt.finished:
                    break
                else:
                    pass
        rpc_response = reply_evt.result
        if rpc_response["status"] != reply_evt.status_opt.SUCCESS:
            raise FastApiHTTPException(
                status_code=FastApiHTTPstatus.HTTP_503_SERVICE_UNAVAILABLE,
                headers={},
                detail={"app_code": [AppCodeOptions.user_management.value[0]]},
            )
        validated_staff_ids = rpc_response["result"]
        diff = set(staff_ids) - set(validated_staff_ids)
        if any(diff):
            err_detail = {
                "code": "invalid_descendant",
                "supervisor_id": supervisor_id,
                "staff_ids": list(diff),
            }
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
        return validated_staff_ids


class BusinessHoursDaysReqBody(PydanticRootModel[List[BusinessHoursDayReqBody]]):
    model_config = ConfigDict(from_attributes=True)

    @field_validator("root")
    def validate_list_items(cls, values):
        days = set(map(lambda obj: obj.day, values))
        if len(days) != len(values):
            err_detail = {"code": "duplicate", "field": ["day"]}
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
        return values


class EditProductReqBody(PydanticBaseModel):
    product_type: SaleableTypeEnum
    product_id: PositiveInt
    price: PositiveInt
    start_after: datetime
    end_before: datetime
    model_config = ConfigDict(from_attributes=True)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # MariaDB DATETIME is not allowed to save time zone, currently should be removed.
        # TODO, keep the time zone of invididual product if required
        # self._tz_start_after = self.start_after.tzinfo
        # self._tz_end_before  = self.end_before.tzinfo
        self.start_after = self.start_after.replace(tzinfo=None)
        self.end_before = self.end_before.replace(tzinfo=None)
        if self.start_after > self.end_before:
            err_detail = {"code": "invalid_time_period"}
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )


class EditProductsReqBody(PydanticRootModel[List[EditProductReqBody]]):
    model_config = ConfigDict(from_attributes=True)

    @field_validator("root")
    def validate_list_items(cls, values):
        prod_ids = set(
            map(lambda obj: (obj.product_type.value, obj.product_id), values)
        )
        if len(prod_ids) != len(values):
            err_detail = {"code": "duplicate", "field": ["product_type", "product_id"]}
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
        return values

    def validate_products(self, staff_id: int):
        filtered = filter(
            lambda obj: obj.product_type == SaleableTypeEnum.ITEM, self.root
        )
        item_ids = list(map(lambda obj: obj.product_id, filtered))
        filtered = filter(
            lambda obj: obj.product_type == SaleableTypeEnum.PACKAGE, self.root
        )
        pkg_ids = list(map(lambda obj: obj.product_id, filtered))
        fields_present = [
            "id",
        ]
        reply_evt = shared_ctx["product_app_rpc"].get_product(
            item_ids=item_ids,
            pkg_ids=pkg_ids,
            profile=staff_id,
            item_fields=fields_present,
            pkg_fields=fields_present,
        )
        if not reply_evt.finished:
            for _ in range(
                shared_ctx["settings"].NUM_RETRY_RPC_RESPONSE
            ):  # TODO, async task
                reply_evt.refresh(retry=False, timeout=0.4, num_of_msgs_fetch=1)
                if reply_evt.finished:
                    break
                else:
                    pass
        rpc_response = reply_evt.result
        if rpc_response["status"] != reply_evt.status_opt.SUCCESS:
            raise FastApiHTTPException(
                status_code=FastApiHTTPstatus.HTTP_503_SERVICE_UNAVAILABLE,
                headers={},
                detail={"app_code": [AppCodeOptions.product.value[0]]},
            )
        validated_data = rpc_response["result"]
        validated_item_ids = set(map(lambda d: d["id"], validated_data["item"]))
        validated_pkg_ids = set(map(lambda d: d["id"], validated_data["pkg"]))
        diff_item = set(item_ids) - validated_item_ids
        diff_pkg = set(pkg_ids) - validated_pkg_ids
        err_detail = {"code": "invalid", "field": []}
        if any(diff_item):
            diff_item = map(
                lambda v: {
                    "product_type": SaleableTypeEnum.ITEM.value,
                    "product_id": v,
                },
                diff_item,
            )
            err_detail["field"].extend(list(diff_item))
        if any(diff_pkg):
            diff_pkg = map(
                lambda v: {
                    "product_type": SaleableTypeEnum.PACKAGE.value,
                    "product_id": v,
                },
                diff_pkg,
            )
            err_detail["field"].extend(list(diff_pkg))
        if any(err_detail["field"]):
            raise FastApiHTTPException(
                detail=err_detail,
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )


## end of class EditProductsReqBody


def _get_quota_arrangement_helper(
    supervisor_verified: dict, req_prof_id: int, out: dict
):
    err_detail = []
    item = supervisor_verified.get(req_prof_id, None)
    if item:
        auth_status = item.get("auth", ActivationStatus.ACCOUNT_NON_EXISTENT.value)
        if auth_status != ActivationStatus.ACCOUNT_ACTIVATED.value:
            err_detail.append("unable to login")
        if out.get(req_prof_id):
            log_args = ["action", "duplicate-quota", "req_prof_id", str(req_prof_id)]
            _logger.warning(None, *log_args)
        else:
            quota_material_models = (StoreProfile, StoreEmail, StorePhone)
            out[req_prof_id] = {}
            quota = item.get("quota", [])
            filter_fn = (
                lambda d, model_cls: d["mat_code"] == model_cls.quota_material.value
            )
            for model_cls in quota_material_models:
                bound_filter_fn = partial(filter_fn, model_cls=model_cls)
                filtered = tuple(filter(bound_filter_fn, quota))
                maxnum = filtered[0]["maxnum"] if any(filtered) else 0
                out[req_prof_id][model_cls] = maxnum
    else:
        err_detail.append("non-existent user profile")
    err_detail = {"supervisor_id": err_detail} if any(err_detail) else {}
    return err_detail
