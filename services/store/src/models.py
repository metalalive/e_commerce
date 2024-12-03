import enum
from functools import partial
from typing import List, Tuple, Self
from sqlalchemy import (
    Column,
    Boolean,
    SmallInteger,
    Enum as sqlalchemy_enum,
    String,
    Text,
    DateTime,
    Time,
    ForeignKey,
    delete as SqlAlDelete,
    select as SqlAlSelect,
    or_ as SqlAlOr,
    and_ as SqlAlAnd,
)
from sqlalchemy import func as sa_func, select as sa_select
from sqlalchemy.event import listens_for
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import declarative_base, relationship
from sqlalchemy.orm.attributes import set_committed_value
from sqlalchemy.inspection import inspect as sa_inspect
from sqlalchemy.dialects.mysql import INTEGER as MYSQL_INTEGER

from ecommerce_common.models.mixins import IdGapNumberFinder

from .dto import EnumWeekDay, CountryCodeEnum

Base = declarative_base()


# TODO, make the material code configurable
class _MatCodeOptions(enum.Enum):
    MAX_NUM_STORES = 1
    MAX_NUM_STAFF = 2
    MAX_NUM_EMAILS = 3
    MAX_NUM_PHONES = 4
    MAX_NUM_PRODUCTS = 5


class EmailMixin:
    # subclasses should extend the white list based on application requirements
    email_domain_whitelist = ["localhost"]
    # NOTE, validation is handled at API view level, not at model level
    addr = Column(
        String(160),
        nullable=False,
    )


class PhoneMixin:
    # NOTE, validation is handled at API view level, not at model level
    country_code = Column(String(3), nullable=False)
    line_number = Column(String(15), nullable=False)


class LocationMixin:
    # billing / shipping address for buyer recieving invoice / receipt / purchased items from seller.
    # the content comes from either snapshot of buyer's location in user_management app
    # or custom location  only for specific order.
    # The snapshot is essential in case the buyer modifies the contact after he/she creates the order.
    country = Column(sqlalchemy_enum(CountryCodeEnum), nullable=False)
    locality = Column(String(50), nullable=False)
    street = Column(String(50), nullable=False)
    detail = Column(Text)
    floor = Column(SmallInteger, default=1, nullable=False)


class AppIdGapNumberFinder(IdGapNumberFinder):
    def __new__(cls, orm_model_class, session, *args, **kwargs):
        instance = super().__new__(cls, orm_model_class, *args, **kwargs)
        instance._session = session
        return instance

    def expected_db_errors(self):
        return (IntegrityError,)

    def is_db_err_recoverable(self, error) -> bool:
        msg = error.args[0]
        recoverable = "Duplicate entry" in msg and "PRIMARY" in msg
        return recoverable

    async def async_lowlvl_gap_range(self, raw_sql_queries) -> list:
        out = []
        conn = self._session.bind
        # TODO, figure out how to retrieve async cursor from async connection
        # NOTE
        # - In synchronous connection, you can get low-level cursor without too
        #   much difficulty, so it is possible to execute multiple different SQL
        #   statements in one network flight. Also the cursor relies on multi-statement
        #   flag (in low-level database) to be set in database server, and it is
        #   essential to ensure all the inputs to the SQL statements are trusted
        # - currently SQLAlchemy does not seem to support async cursor from async
        #   connection, it will always report errors like `sqlalchemy.exc.MissingGreenlet`
        #   or `sqlalchemy.exc.InvalidRequestError` when attempting to invoke `cursor()`
        #   method
        resultset = await conn.exec_driver_sql(raw_sql_queries[0])
        row = resultset.one_or_none()
        if row:
            out.append(row)
        resultset = await conn.exec_driver_sql(raw_sql_queries[1])
        out.extend(resultset.fetchall())
        resultset = await conn.exec_driver_sql(raw_sql_queries[2])
        row = resultset.fetchone()
        if row:
            out.append(row)
        return out

    def get_pk_db_column(self, model_cls) -> str:
        return sa_inspect(model_cls).primary_key[0].name

    def get_db_table_name(self, model_cls) -> str:
        return model_cls.__table__.name

    def extract_dup_id_from_error(self, error):
        raw_msg_chars = error.args[0].split()
        idx = raw_msg_chars.index("entry")
        dup_id = raw_msg_chars[idx + 1]
        dup_id = dup_id.strip("'")
        dup_id = int(dup_id)
        return dup_id


class QuotaStatisticsMixin:
    @classmethod
    def get_existing_items_stats(cls, target_ids, attname):
        attr = getattr(cls, attname)
        query = sa_select(attr, sa_func.count(attr))
        query = query.group_by(attr)
        return query.filter(attr.in_(target_ids))

    @classmethod
    async def quota_stats(cls, objs, session, target_ids, attname):
        # class-level validation is used for checking several instances at once
        # Note that quota has to come from trusted source e.g. authenticated JWT payload
        attr_fd = getattr(cls, attname)
        base_query = cls.get_existing_items_stats(target_ids, attname)
        result = {}
        for target_id in target_ids:
            new_items = tuple(
                filter(lambda obj: getattr(obj, attname) == target_id, objs)
            )
            num_new_items = len(new_items)
            query = base_query.filter(attr_fd == target_id)
            resultset = await session.execute(query)
            existing_items = resultset.one_or_none()
            existing_items = existing_items or (target_id, 0)
            num_existing_items = existing_items[1]
            result[target_id] = {
                "num_new_items": num_new_items,
                "num_existing_items": num_existing_items,
            }
        return result


class StoreCurrency(enum.Enum):
    TWD = "TWD"
    INR = "INR"
    IDR = "IDR"
    THB = "THB"
    USD = "USD"


# note that an organization (e.g. a company) can have several stores (either outlet or online)
class StoreProfile(Base, QuotaStatisticsMixin):
    quota_material = _MatCodeOptions.MAX_NUM_STORES
    __tablename__ = "store_profile"

    id = Column(MYSQL_INTEGER(unsigned=True), primary_key=True, autoincrement=False)
    label = Column(String(32), nullable=False)
    # come from GenericUserProfile in user_management app
    supervisor_id = Column(MYSQL_INTEGER(unsigned=True), autoincrement=False)
    active = Column(Boolean)
    currency = Column(sqlalchemy_enum(StoreCurrency), nullable=False)
    # bidirectional relationship has to be declared on both sides of the models
    emails = relationship(
        "StoreEmail",
        back_populates="store_applied",
        cascade="save-update, merge, delete, delete-orphan",
    )
    phones = relationship(
        "StorePhone",
        back_populates="store_applied",
        cascade="save-update, merge, delete, delete-orphan",
    )
    location = relationship(
        "OutletLocation",
        back_populates="store_applied",
        cascade="save-update, merge, delete, delete-orphan",
        uselist=False,
    )
    open_days = relationship(
        "HourOfOperation",
        back_populates="store_applied",
        cascade="save-update, merge, delete, delete-orphan",
    )
    staff = relationship(
        "StoreStaff",
        back_populates="store_applied",
        cascade="save-update, merge, delete, delete-orphan",
    )
    products = relationship(
        "StoreProductAvailable",
        back_populates="store_applied",
        cascade="save-update, merge, delete, delete-orphan",
    )

    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(
            objs, session, target_ids, attname="supervisor_id"
        )

    @classmethod
    async def bulk_insert(cls, objs, session):
        async def save_instance_fn():
            try:
                session.add_all(objs)  # check state in session.new
                await session.commit()
            except Exception:
                # manually rollback, change random ID, then commit again
                await session.rollback()
                raise

        _id_gap_finder = AppIdGapNumberFinder(orm_model_class=cls, session=session)
        await _id_gap_finder.async_save_with_rand_id(save_instance_fn, objs=objs)


## end of class StoreProfile


@listens_for(StoreProfile, "before_update")
@listens_for(StoreProfile, "before_insert")
def _reset_seq_num(mapper, conn, target):
    # set_committed_value() is used to avoid the SAwarning described in https://stackoverflow.com/q/38074244/9853105
    for idx in range(len(target.emails)):
        email = target.emails[idx]
        set_committed_value(email, "seq", idx)
    for idx in range(len(target.phones)):
        phone = target.phones[idx]
        set_committed_value(phone, "seq", idx)


class TimePeriodValidMixin:
    # time period at which an entity is valid in a store
    # (e.g. a saleable item or a staff is available)
    start_after = Column(DateTime, nullable=True)
    end_before = Column(DateTime, nullable=True)


class StoreStaff(Base, TimePeriodValidMixin, QuotaStatisticsMixin):
    quota_material = _MatCodeOptions.MAX_NUM_STAFF
    __tablename__ = "store_staff"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    # come from GenericUserProfile in user_management app
    staff_id = Column(
        MYSQL_INTEGER(unsigned=True), primary_key=True, autoincrement=False
    )
    store_applied = relationship("StoreProfile", back_populates="staff")

    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="store_id")


class StoreEmail(Base, EmailMixin, QuotaStatisticsMixin):
    quota_material = _MatCodeOptions.MAX_NUM_EMAILS
    __tablename__ = "store_email"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    seq = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)
    store_applied = relationship("StoreProfile", back_populates="emails")

    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="store_id")


class StorePhone(Base, PhoneMixin, QuotaStatisticsMixin):
    quota_material = _MatCodeOptions.MAX_NUM_PHONES
    __tablename__ = "store_phone"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    seq = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)
    store_applied = relationship("StoreProfile", back_populates="phones")

    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="store_id")


class OutletLocation(Base, LocationMixin):
    __tablename__ = "outlet_location"
    # a store includes only one outlet , or no outlet if it goes online
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    store_applied = relationship("StoreProfile", back_populates="location")


class SaleableTypeEnum(enum.Enum):
    ITEM = 1
    PACKAGE = 2


class StoreProductAvailable(Base, TimePeriodValidMixin, QuotaStatisticsMixin):
    quota_material = _MatCodeOptions.MAX_NUM_PRODUCTS
    __tablename__ = "store_product_available"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    # following 2 fields come from product app
    product_type = Column(sqlalchemy_enum(SaleableTypeEnum), primary_key=True)
    product_id = Column(MYSQL_INTEGER(unsigned=True), primary_key=True)
    price = Column(MYSQL_INTEGER(unsigned=True), nullable=False)
    store_applied = relationship("StoreProfile", back_populates="products")

    # NOTE, don't record inventory data at this app, do it in inventory app
    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="store_id")

    @classmethod
    async def try_load(cls, session, store_id: int, reqdata: List) -> List[Self]:
        def cond_clause_and(d):
            return SqlAlAnd(
                cls.product_type == d.product_type,
                cls.product_id == d.product_id,
            )

        product_id_cond = map(cond_clause_and, reqdata)
        find_product_condition = SqlAlOr(*product_id_cond)
        ## Don't use `saved_obj.products` generated by SQLAlchemy legacy Query API
        ## , instead I use `select` function to query relation fields
        stmt = (
            SqlAlSelect(cls)
            .where(cls.store_id == store_id)
            .where(find_product_condition)
        )
        resultset = await session.execute(stmt)
        objs = [p[0] for p in resultset]  # tuple
        return objs

    @staticmethod
    def bulk_update(objs: List[Self], reqdata: List) -> List[Tuple[enum.Enum, int]]:
        def _do_update(obj):
            def check_prod_id(d) -> bool:
                return (
                    d.product_type is obj.product_type
                    and d.product_id == obj.product_id
                )

            newdata = next(filter(check_prod_id, reqdata))
            assert newdata is not None
            obj.price = newdata.price
            obj.start_after = newdata.start_after
            obj.end_before = newdata.end_before
            return (newdata.product_type, newdata.product_id)

        return list(map(_do_update, objs))

    @classmethod
    async def bulk_delete(
        cls, session, store_id: int, item_ids: List[int], pkg_ids: List[int]
    ) -> int:
        def _cond_fn(d, t):
            return SqlAlAnd(cls.product_type == t, cls.product_id == d)

        pitem_cond = map(partial(_cond_fn, t=SaleableTypeEnum.ITEM), item_ids)
        ppkg_cond = map(partial(_cond_fn, t=SaleableTypeEnum.PACKAGE), pkg_ids)
        find_product_condition = SqlAlOr(*pitem_cond, *ppkg_cond)
        stmt = (
            SqlAlDelete(cls)
            .where(cls.store_id == store_id)
            .where(find_product_condition)
        )
        result = await session.execute(stmt)
        return result.rowcount


class HourOfOperation(Base):
    """weekly business hours for each store"""

    __tablename__ = "hour_of_operation"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    day = Column(sqlalchemy_enum(EnumWeekDay), primary_key=True)
    time_open = Column(Time, nullable=False)
    time_close = Column(Time, nullable=False)
    store_applied = relationship("StoreProfile", back_populates="open_days")
