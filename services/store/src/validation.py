import logging
from datetime import datetime
from functools import partial
from typing import List, Dict

from pydantic import (
    BaseModel as PydanticBaseModel,
    RootModel as PydanticRootModel,
    PositiveInt,
    field_validator,
    ConfigDict,
)

from ecommerce_common.models.enums.base import AppCodeOptions, ActivationStatus
from .dto import (
    NewStoreProfileDto,
    StoreStaffDto,
    StoreDtoError,
    BusinessHoursDayDto,
    EditProductDto,
    QuotaMatCode,
)

_logger = logging.getLogger(__name__)


def _get_supervisor_auth(prof_ids, shr_ctx):  # TODO, async operation
    reply_evt = shr_ctx["auth_app_rpc"].get_profile(
        ids=prof_ids, fields=["id", "auth", "quota"]
    )
    num_entry = shr_ctx["settings"].NUM_RETRY_RPC_RESPONSE
    if not reply_evt.finished:
        for _ in range(num_entry):  # TODO, async task
            reply_evt.refresh(retry=False, timeout=0.5, num_of_msgs_fetch=1)
            if reply_evt.finished:
                break
            else:
                pass
    rpc_response = reply_evt.result
    if rpc_response["status"] != reply_evt.status_opt.SUCCESS:
        raise shr_ctx.rpc_error(
            detail={"app_code": [AppCodeOptions.user_management.value[0]]}
        )
    return rpc_response["result"]


class NewStoreProfilesReqBody(PydanticRootModel[List[NewStoreProfileDto]]):
    @field_validator("root")  # map to default field name in the root-model
    def validate_list_items(cls, values):
        assert values and any(values), "Empty request body Not Allowed"
        return values

    def check_existence(self, shr_ctx):
        req_prof_ids = self.supervisor_profile_ids()
        supervisor_verified = _get_supervisor_auth(req_prof_ids, shr_ctx)
        quota_arrangement = self._estimate_quota(supervisor_verified)
        self._contact_common_quota_check(
            quota_arrangement,
            label="emails",
            mat_code=QuotaMatCode.MAX_NUM_EMAILS,
        )
        self._contact_common_quota_check(
            quota_arrangement,
            label="phones",
            mat_code=QuotaMatCode.MAX_NUM_PHONES,
        )

    def _estimate_quota(self, supervisor_verified):
        supervisor_verified = {item["id"]: item for item in supervisor_verified}
        out = {}

        def _fn(item):
            err = _get_quota_arrangement_helper(
                supervisor_verified, req_prof_id=item.supervisor_id, out=out
            )
            if not any(err):
                item.quota = out[item.supervisor_id]
            return err

        err_content = list(map(_fn, self.root))
        if any(err_content):
            raise StoreDtoError(detail=err_content, perm=False)
        return out

    def _contact_common_quota_check(
        self,
        quota_arrangement: dict,
        label: str,
        mat_code: QuotaMatCode,
    ):
        def _inner_chk(item):
            err = {}
            num_new_items = len(getattr(item, label))
            max_limit = quota_arrangement[item.supervisor_id][mat_code]
            if max_limit < num_new_items:
                err["supervisor_id"] = item.supervisor_id
                err[label] = {
                    "type": "limit-exceed",
                    "max_limit": max_limit,
                    "num_new_items": num_new_items,
                }
            return err

        err_content = list(map(_inner_chk, self.root))
        if any(err_content):
            raise StoreDtoError(detail=err_content, perm=True)

    def supervisor_profile_ids(self) -> List[int]:
        return list(map(lambda obj: obj.supervisor_id, self.root))

    def validate_quota(self, quota_chk_result: Dict):
        # quota check, for current user who adds these new items
        quota_arrangement = {obj.supervisor_id: obj.quota for obj in self.root}

        def _inner_chk(item):
            err = {}
            num_existing_items = quota_chk_result[item.supervisor_id][
                "num_existing_items"
            ]
            num_new_items = quota_chk_result[item.supervisor_id]["num_new_items"]
            curr_used = num_existing_items + num_new_items
            max_limit = quota_arrangement[item.supervisor_id][
                QuotaMatCode.MAX_NUM_STORES
            ]
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
            raise StoreDtoError(detail=err_content, perm=True)


## end of class NewStoreProfilesReqBody()


class StoreSupervisorReqBody(PydanticBaseModel):
    supervisor_id: PositiveInt  # for new supervisor

    def check_existence(self, shr_ctx):
        req_prof_id = self.supervisor_id
        supervisor_verified = _get_supervisor_auth([req_prof_id], shr_ctx)
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
            raise StoreDtoError(detail=err_detail, perm=False)
        return out

    def validate_quota(self, quota_chk_result):
        prof_id = self.supervisor_id
        quota_arrangement = self.metadata["quota_arrangement"]
        err = {}
        num_existing_items = quota_chk_result[prof_id]["num_existing_items"]
        num_new_items = 1
        curr_used = num_existing_items + num_new_items
        max_limit = quota_arrangement[prof_id][QuotaMatCode.MAX_NUM_STORES]
        if max_limit < curr_used:
            err["supervisor_id"] = prof_id
            err["store_profile"] = {
                "type": "limit-exceed",
                "max_limit": max_limit,
                "num_new_items": num_new_items,
                "num_existing_items": num_existing_items,
            }
        if any(err):
            raise StoreDtoError(detail=err, perm=True)


class StoreStaffsReqBody(PydanticRootModel[List[StoreStaffDto]]):
    @field_validator("root")
    def validate_list_items(cls, values):
        staff_ids = set(map(lambda obj: obj.staff_id, values))
        if len(staff_ids) != len(values):
            err_detail = {"code": "duplicate", "field": ["staff_id"]}
            raise StoreDtoError(detail=err_detail, perm=False)
        return values

    def validate_staff(self, shr_ctx, supervisor_id: int):
        staff_ids = list(map(lambda obj: obj.staff_id, self.root))
        reply_evt = shr_ctx["auth_app_rpc"].profile_descendant_validity(
            asc=supervisor_id, descs=staff_ids
        )
        num_retry = shr_ctx["settings"].NUM_RETRY_RPC_RESPONSE
        if not reply_evt.finished:
            for _ in range(num_retry):  # TODO, async task
                reply_evt.refresh(retry=False, timeout=0.4, num_of_msgs_fetch=1)
                if reply_evt.finished:
                    break
        rpc_response = reply_evt.result
        if rpc_response["status"] != reply_evt.status_opt.SUCCESS:
            raise shr_ctx.rpc_error(
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
            raise StoreDtoError(detail=err_detail, perm=False)
        return validated_staff_ids


class BusinessHoursDaysReqBody(PydanticRootModel[List[BusinessHoursDayDto]]):
    model_config = ConfigDict(from_attributes=True)

    @field_validator("root")
    def validate_list_items(cls, values):
        days = set(map(lambda obj: obj.day, values))
        if len(days) != len(values):
            err_detail = {"code": "duplicate", "field": ["day"]}
            raise StoreDtoError(detail=err_detail, perm=False)
        return values


class EditProductsReqBody(PydanticRootModel[List[EditProductDto]]):
    model_config = ConfigDict(from_attributes=True)

    @field_validator("root")
    def validate_list_items(cls, values):
        prod_ids = set(map(lambda obj: obj.product_id, values))
        if len(prod_ids) != len(values):
            err_detail = {"code": "duplicate", "field": ["product_id"]}
            raise StoreDtoError(detail=err_detail, perm=False)
        return values

    def validate_products(self, shr_ctx, staff_id: int):
        # TODO, refactor RPC and relevant validation
        item_ids = list(map(lambda obj: obj.product_id, self.root))
        cls = type(self)
        valid_data = cls.refresh_product_attributes(
            shr_ctx,
            product_ids=item_ids,
            staff_id=staff_id,
        )
        diff_item = set(item_ids) - set(valid_data.keys())
        err_detail = {"code": "invalid", "field": []}
        if any(diff_item):
            diff_item = map(lambda v: {"product_id": v}, diff_item)
            err_detail["field"].extend(list(diff_item))
        if any(err_detail["field"]):
            raise StoreDtoError(detail=err_detail, perm=False)
        for obj in self.root:
            valid_attris = valid_data.get(obj.product_id)
            obj.validate_attr(valid_attris)

    @staticmethod
    def refresh_product_attributes(
        shr_ctx, product_ids: List[int], staff_id: int
    ) -> Dict[int, Dict]:
        reply_evt = shr_ctx["product_app_rpc"].get_product(
            item_ids=product_ids, profile=staff_id
        )
        num_retry = shr_ctx["settings"].NUM_RETRY_RPC_RESPONSE
        if not reply_evt.finished:
            for _ in range(num_retry):  # TODO, async task
                reply_evt.refresh(retry=False, timeout=0.4, num_of_msgs_fetch=1)
                if reply_evt.finished:
                    break
        rpc_response = reply_evt.result
        if rpc_response["status"] != reply_evt.status_opt.SUCCESS:
            raise shr_ctx.rpc_error(
                detail={"app_code": [AppCodeOptions.product.value[0]]}
            )
        raw = rpc_response["result"]["result"]
        return {
            d["id_"]: {
                "attributes": {
                    (v["label"]["id_"], v["value"]) for v in d["attributes"]
                },
                "last_update": datetime.fromisoformat(d["last_update"]),
            }
            for d in raw
        }


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
            quota_material_codes = (
                QuotaMatCode.MAX_NUM_STORES,
                QuotaMatCode.MAX_NUM_EMAILS,
                QuotaMatCode.MAX_NUM_PHONES,
            )
            out[req_prof_id] = {}
            quota = item.get("quota", [])

            def filter_fn(d, given_code) -> bool:
                return d["mat_code"] == given_code.value

            for code in quota_material_codes:
                bound_filter_fn = partial(filter_fn, given_code=code)
                filtered = tuple(filter(bound_filter_fn, quota))
                maxnum = filtered[0]["maxnum"] if any(filtered) else 0
                out[req_prof_id][code] = maxnum
    else:
        err_detail.append("non-existent user profile")
    err_detail = {"supervisor_id": err_detail} if any(err_detail) else {}
    return err_detail
