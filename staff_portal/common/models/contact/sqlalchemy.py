import enum

from sqlalchemy import Column, SmallInteger, Enum as sqlalchemy_enum, String, Text
from sqlalchemy.orm import validates

from common.models.enums.base import JsonFileChoicesMeta

class EmailMixin:
    # subclasses should extend the white list based on application requirements
    email_domain_whitelist = ['localhost']
    # NOTE, validation is handled at API view level, not at model level
    addr = Column(String(160), nullable=False,)


class PhoneMixin:
    # NOTE, validation is handled at API view level, not at model level
    country_code = Column(String(3 ), nullable=False)
    line_number  = Column(String(15), nullable=False)
    regex_patt = {
        'country_code': r"^\d{1,3}$",
        'line_number': r"^\+?1?\d{7,15}$",
    }
    err_msg = {
        'country_code': "non-digit character detected, or length of digits doesn't meet requirement. It must contain only digit e.g. '91', '886' , from 1 digit up to 3 digits",
        'line_number': "non-digit character detected, or length of digits doesn't meet requirement. It must contain only digits e.g. '9990099', from 7 digits up to 15 digits",
    }




class CountryCodeEnum(enum.Enum, metaclass=JsonFileChoicesMeta):
    filepath = 'common/data/nationality_code.json'

class LocationMixin:
    # billing / shipping address for buyer recieving invoice / receipt / purchased items from seller.
    # the content comes from either snapshot of buyer's location in user_management app
    # or custom location  only for specific order.
    # The snapshot is essential in case the buyer modifies the contact after he/she creates the order.
    country  = Column(sqlalchemy_enum(CountryCodeEnum), nullable=False)
    locality = Column(String(50), nullable=False)
    street   = Column(String(50), nullable=False)
    detail   = Column(Text)
    floor = Column(SmallInteger, default=1, nullable=False)

