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
    StringConstraints,
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
    def __init__(self, detail: Dict):
        self.detail = detail


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
