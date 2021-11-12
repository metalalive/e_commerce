import enum
import calendar
from datetime import datetime

from sqlalchemy import Column, Boolean, Integer, SmallInteger, Enum as sqlalchemy_enum, Float, String, Text, DateTime, Time, ForeignKey
from sqlalchemy.orm import declarative_base, declarative_mixin, declared_attr

from common.models.contact.sqlalchemy import EmailMixin, PhoneMixin, LocationMixin

from . import settings

Base = declarative_base()

# note that an organization (e.g. a company) can have several stores (either outlet or online)

class StoreProfile(Base):
    quota_material = settings._MatCodeOptions.MAX_NUM_STORES
    __tablename__ = 'store_profile'

    def _rand_gen_id(self):
        raise NotImplementedError()

    id = Column(Integer, primary_key=True, autoincrement=False, default=_rand_gen_id)
    label = Column(String(32), nullable=False)
    # come from GenericUserProfile in user_management app
    superviser_id = Column(Integer, autoincrement=False)
    active = Column(Boolean)


class TimePeriodValidMixin:
    # time period at which an entity is valid in a store
    # (e.g. a saleable item or a staff is available)
    start_after = Column(DateTime, nullable=True)
    end_before  = Column(DateTime, nullable=True)


class StoreStaff(Base, TimePeriodValidMixin):
    quota_material = settings._MatCodeOptions.MAX_NUM_STAFF
    __tablename__ = 'store_staff'
    store_id  = Column(Integer, ForeignKey('store_profile.id'), primary_key=True)
    # come from GenericUserProfile in user_management app
    staff_id = Column(Integer, primary_key=True, autoincrement=False)


class StoreEmail(Base, EmailMixin):
    quota_material = settings._MatCodeOptions.MAX_NUM_EMAILS
    __tablename__ = 'store_email'
    store_id  = Column(Integer, ForeignKey('store_profile.id'), primary_key=True)
    seq  = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)

class StorePhone(Base, PhoneMixin):
    quota_material = settings._MatCodeOptions.MAX_NUM_PHONES
    __tablename__ = 'store_phone'
    store_id  = Column(Integer, ForeignKey('store_profile.id'), primary_key=True)
    seq  = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)

class OutletLocation(Base, LocationMixin):
    __tablename__ = 'outlet_location'
    # a store includes only one outlet , or no outlet if it goes online
    store_id  = Column(Integer, ForeignKey('store_profile.id'), primary_key=True)


class _SaleableTypeEnum(enum.Enum):
    ITEM = 1
    PACKAGE = 2


class StoreProductAvailable(Base, TimePeriodValidMixin):
    quota_material = settings._MatCodeOptions.MAX_NUM_PRODUCTS
    __tablename__ = 'store_product_avaiable'
    store_id  = Column(Integer, ForeignKey('store_profile.id'), primary_key=True)
    # following 2 fields come from product app
    product_type = Column(sqlalchemy_enum(_SaleableTypeEnum), primary_key=True)
    product_id = Column(Integer, primary_key=True)
    # NOTE, don't record inventory data at this app, do it in inventory app


class EnumWeekDay(enum.Enum):
    SUNDAY  = calendar.SUNDAY
    MONDAY  = calendar.MONDAY
    TUESDAY = calendar.TUESDAY
    WEDNESDAY = calendar.WEDNESDAY
    THURSDAY = calendar.THURSDAY
    FRIDAY   = calendar.FRIDAY
    SATURDAY = calendar.SATURDAY


class HourOfOperation(Base):
    """ weekly business hours for each store """
    __tablename__ = 'hour_of_operation'
    store_id  = Column(Integer, ForeignKey('store_profile.id'), primary_key=True)
    day = Column(sqlalchemy_enum(EnumWeekDay), primary_key=True)
    time_open  = Column(Time, nullable=False)
    time_close = Column(Time, nullable=False)

