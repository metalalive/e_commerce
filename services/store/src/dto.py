import calendar
import enum
from datetime import datetime, time as py_time
from typing import Optional, List, Dict, Union
from typing_extensions import Annotated

from pydantic import (
    BaseModel as PydanticBaseModel,
    ConfigDict,
    EmailStr,
    PositiveInt,
    NonNegativeInt,
    StringConstraints,
    SkipValidation,
)

from ecommerce_common.models.enums.base import JsonFileChoicesMeta


class EnumWeekDay(enum.Enum):
    SUNDAY = calendar.SUNDAY
    MONDAY = calendar.MONDAY
    TUESDAY = calendar.TUESDAY
    WEDNESDAY = calendar.WEDNESDAY
    THURSDAY = calendar.THURSDAY
    FRIDAY = calendar.FRIDAY
    SATURDAY = calendar.SATURDAY


class CountryCodeEnum(enum.Enum, metaclass=JsonFileChoicesMeta):
    filepath = "common/data/nationality_code.json"


class StoreCurrency(enum.Enum):
    TWD = "TWD"
    INR = "INR"
    IDR = "IDR"
    THB = "THB"
    USD = "USD"


# TODO, make the material code configurable
class QuotaMatCode(enum.Enum):
    MAX_NUM_STORES = 1
    MAX_NUM_STAFF = 2
    MAX_NUM_EMAILS = 3
    MAX_NUM_PHONES = 4
    MAX_NUM_PRODUCTS = 5


class StoreEmailDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    addr: EmailStr


class StorePhoneDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    country_code: Annotated[
        str,
        StringConstraints(
            strip_whitespace=False, to_upper=False, to_lower=False, pattern=r"^\d{1,3}$"
        ),
    ]
    line_number: Annotated[
        str,
        StringConstraints(
            strip_whitespace=False,
            to_upper=False,
            to_lower=False,
            pattern=r"^\+?1?\d{7,15}$",
        ),
    ]


class ShopLocationDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    country: CountryCodeEnum
    # TODO
    # - split `locality` to 2 fields `city` and `state` (a.k.a. province, region)
    # - add new field `postal_code`
    locality: str
    street: str
    detail: str
    floor: int


class StoreDtoError(Exception):
    def __init__(self, detail: Dict, perm: bool):
        self.detail = detail
        self._permission_failure = perm

    @property
    def permission(self) -> bool:
        return self._permission_failure


class StoreStaffDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    staff_id: PositiveInt
    start_after: datetime
    end_before: datetime

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # MariaDB DATETIME is not allowed to save time zone, currently should be removed.
        # TODO, keep the time zone of invididual product if required
        self.start_after = self.start_after.replace(tzinfo=None)
        self.end_before = self.end_before.replace(tzinfo=None)
        if self.start_after > self.end_before:
            err_detail = {"code": "invalid_time_period"}
            raise StoreDtoError(err_detail)


class BusinessHoursDayDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    day: EnumWeekDay
    time_open: py_time
    time_close: py_time

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if self.time_open > self.time_close:
            err_detail = {"code": "invalid_time_period"}
            raise StoreDtoError(err_detail)


class NewStoreProfileDto(PydanticBaseModel):
    label: str
    supervisor_id: PositiveInt
    currency: StoreCurrency
    active: Optional[bool] = False
    emails: Optional[List[StoreEmailDto]] = []
    phones: Optional[List[StorePhoneDto]] = []
    location: Optional[ShopLocationDto] = None
    # quota should be updated by `NewStoreProfilesReqBody`
    quota: SkipValidation[Optional[Dict]] = None


class EditExistingStoreProfileDto(PydanticBaseModel):
    label: str
    active: bool
    currency: StoreCurrency
    emails: Optional[List[StoreEmailDto]] = []
    phones: Optional[List[StorePhoneDto]] = []
    location: Optional[ShopLocationDto] = None


class StoreProfileCreatedDto(PydanticBaseModel):
    id: PositiveInt
    supervisor_id: PositiveInt


class StoreProfileDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    label: str
    active: bool
    supervisor_id: PositiveInt
    emails: Optional[List[StoreEmailDto]] = []
    phones: Optional[List[StorePhoneDto]] = []
    location: Optional[ShopLocationDto] = None
    staff: Optional[List[StoreStaffDto]] = []
    open_days: Optional[List[BusinessHoursDayDto]] = []


class ProductAttrPriceDto(PydanticBaseModel):
    model_config = ConfigDict(from_attributes=True)
    label_id: str
    value: Union[bool, NonNegativeInt, int, str]
    price: NonNegativeInt  # extra amount to charge


class EditProductDto(PydanticBaseModel):
    product_id: PositiveInt
    base_price: PositiveInt
    start_after: datetime
    end_before: datetime
    attrs_charge: List[ProductAttrPriceDto]
    model_config = ConfigDict(from_attributes=True)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        NUM_ATTR_HARD_LIMIT = 20
        if len(self.attrs_charge) > NUM_ATTR_HARD_LIMIT:
            err_detail = {"code": "num_attrs_exceed", "limit": NUM_ATTR_HARD_LIMIT}
            raise StoreDtoError(detail=err_detail)
        # MariaDB DATETIME is not allowed to save time zone, currently should be removed.
        # TODO, keep the time zone of invididual product if required
        # self._tz_start_after = self.start_after.tzinfo
        # self._tz_end_before  = self.end_before.tzinfo
        self.start_after = self.start_after.replace(tzinfo=None)
        self.end_before = self.end_before.replace(tzinfo=None)
        self._attr_last_update = None
        if self.start_after > self.end_before:
            err_detail = {"code": "invalid_time_period"}
            raise StoreDtoError(detail=err_detail)

    def validate_attr(self, limit: Dict):
        total_req = {(a.label_id, a.value) for a in self.attrs_charge}
        diff = total_req - limit["attributes"]
        if any(diff):
            err_detail = [{"label_id": d[0], "value": d[1]} for d in diff]
            raise StoreDtoError(
                detail={"code": "invalid", "field": {"attributes": err_detail}},
            )
        self._attr_last_update = limit["last_update"]

    @property
    def attribute_lastupdate(self) -> Optional[datetime]:
        return self._attr_last_update
