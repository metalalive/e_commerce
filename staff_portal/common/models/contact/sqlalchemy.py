import enum

from sqlalchemy import Column, SmallInteger, Enum as sqlalchemy_enum, String, Text
from sqlalchemy.orm import validates

from common.models.enums.base import JsonFileChoicesMeta


class EmailMixin:
    # subclasses should extend the white list based on application requirements
    domain_whitelist = ['localhost']
    # the content comes from either snapshot of buyer's email in user_management app
    # or custom email address only for specific order.
    # The snapshot is essential in case the buyer modifies the contact after he/she creates the order
    addr = Column(String(160), nullable=False,)

    @validates('addr')
    def _validate_email_addr(self, key, value):
        err_msg = 'invalid email format'
        user_part, domain_part = value.rsplit('@', 1)
        user_regex = _lazy_re_compile(
            r"(^[-!#$%&'*+/=?^_`{}|~0-9A-Z]+(\.[-!#$%&'*+/=?^_`{}|~0-9A-Z]+)*\Z"  # dot-atom
            r'|^"([\001-\010\013\014\016-\037!#-\[\]-\177]|\\[\001-\011\013\014\016-\177])*"\Z)',  # quoted-string
            re.IGNORECASE)
        if not user_regex.match(user_part):
            raise ValueError(err_msg)
        if (domain_part not in self.domain_whitelist and
                not self.validate_domain_part(domain_part)):
            raise ValueError(err_msg)


class PhoneMixin:
    country_code = Column(String(3 ), nullable=False)
    line_number  = Column(String(15), nullable=False)

    @validates('country_code')
    def _validate_country_code(self, key, value):
        err_msg = "non-digit character detected, or length of digits doesn't meet requirement. It must contain only digit e.g. '91', '886' , from 1 digit up to 3 digits"
        regex_patt = r"^\d{1,3}$"
        raise NotImplementedError()

    @validates('line_number')
    def _validate_line_number(self, key, value):
        err_msg = "non-digit character detected, or length of digits doesn't meet requirement. It must contain only digits e.g. '9990099', from 7 digits up to 15 digits"
        regex_patt = r"^\+?1?\d{7,15}$"
        raise NotImplementedError()



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

