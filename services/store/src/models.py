import enum
import calendar
from datetime import datetime

from sqlalchemy import (
    Column,
    Boolean,
    SmallInteger,
    Enum as sqlalchemy_enum,
    Float,
    String,
    Text,
    DateTime,
    Time,
    ForeignKey,
)
from sqlalchemy import func as sa_func
from sqlalchemy.event import listens_for
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import (
    declarative_base,
    declarative_mixin,
    declared_attr,
    relationship,
)
from sqlalchemy.orm.attributes import set_committed_value
from sqlalchemy.inspection import inspect as sa_inspect
from sqlalchemy.dialects.mysql import INTEGER as MYSQL_INTEGER

from ecommerce_common.models.contact.sqlalchemy import (
    EmailMixin,
    PhoneMixin,
    LocationMixin,
)
from ecommerce_common.models.mixins import IdGapNumberFinder

from settings.common import _MatCodeOptions

Base = declarative_base()


class AppIdGapNumberFinder(IdGapNumberFinder):
    def save_with_rand_id(self, save_instance_fn, objs, session):
        self._session = session
        super().save_with_rand_id(save_instance_fn, objs)
        self._session = None

    def expected_db_errors(self):
        return (IntegrityError,)

    def is_db_err_recoverable(self, error) -> bool:
        mysql_pk_dup_error = (
            lambda x: "Duplicate entry" in x.args[0] and "PRIMARY" in x.args[0]
        )
        recoverable = mysql_pk_dup_error(error)
        return recoverable

    def low_lvl_get_gap_range(self, raw_sql_queries) -> list:
        rawsqls = ";".join(raw_sql_queries)
        out = []
        # get low-level database connection, for retrieving persisted data later without beibg locked
        db_conn = self._session.bind.connection
        # the connection has to be DB-API 2.0 compliant
        # The cursor relies on multi-statement flag (in MySQL) in order to execute multiple raw SQL
        # statements in one go. API developers have to ensure all the inputs to the SQL statements
        # are trusted
        with db_conn.cursor() as cursor:
            cursor.execute(rawsqls)
            row = cursor.fetchone()
            if row:
                out.append(row)
            cursor.nextset()
            out.extend(cursor.fetchall())
            cursor.nextset()
            row = cursor.fetchone()
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
    def get_existing_items_stats(cls, session, target_ids, attname):
        attr = getattr(cls, attname)
        query = session.query(attr, sa_func.count(attr))
        query = query.group_by(attr)
        return query.filter(attr.in_(target_ids))

    @classmethod
    def quota_stats(cls, objs, session, target_ids, attname):
        # class-level validation is used for checking several instances at once
        # Note that quota has to come from trusted source e.g. authenticated JWT payload
        attr_fd = getattr(cls, attname)
        query = cls.get_existing_items_stats(session, target_ids, attname)
        result = {}
        for target_id in target_ids:
            new_items = tuple(
                filter(lambda obj: getattr(obj, attname) == target_id, objs)
            )
            num_new_items = len(new_items)
            existing_items = query.filter(attr_fd == target_id).one_or_none()
            existing_items = existing_items or (target_id, 0)
            num_existing_items = existing_items[1]
            result[target_id] = {
                "num_new_items": num_new_items,
                "num_existing_items": num_existing_items,
            }
        return result


# note that an organization (e.g. a company) can have several stores (either outlet or online)
class StoreProfile(Base, QuotaStatisticsMixin):
    quota_material = _MatCodeOptions.MAX_NUM_STORES
    __tablename__ = "store_profile"

    id = Column(MYSQL_INTEGER(unsigned=True), primary_key=True, autoincrement=False)
    label = Column(String(32), nullable=False)
    # come from GenericUserProfile in user_management app
    supervisor_id = Column(MYSQL_INTEGER(unsigned=True), autoincrement=False)
    active = Column(Boolean)
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
    def quota_stats(cls, objs, session, target_ids):
        return super().quota_stats(objs, session, target_ids, attname="supervisor_id")

    @classmethod
    def bulk_insert(cls, objs, session):
        if not hasattr(cls, "_id_gap_finder"):
            cls._id_gap_finder = AppIdGapNumberFinder(orm_model_class=cls)

        def save_instance_fn():  # TODO, async
            try:
                session.add_all(objs)  # check state in session.new
                session.commit()
            except Exception as e:
                # manually rollback, change random ID, then commit again
                session.rollback()
                raise

        return cls._id_gap_finder.save_with_rand_id(
            save_instance_fn, objs=objs, session=session
        )


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
    def quota_stats(cls, objs, session, target_ids):
        return super().quota_stats(objs, session, target_ids, attname="store_id")


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
    def quota_stats(cls, objs, session, target_ids):
        return super().quota_stats(objs, session, target_ids, attname="store_id")


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
    def quota_stats(cls, objs, session, target_ids):
        return super().quota_stats(objs, session, target_ids, attname="store_id")


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
    def quota_stats(cls, objs, session, target_ids):
        return super().quota_stats(objs, session, target_ids, attname="store_id")


class EnumWeekDay(enum.Enum):
    SUNDAY = calendar.SUNDAY
    MONDAY = calendar.MONDAY
    TUESDAY = calendar.TUESDAY
    WEDNESDAY = calendar.WEDNESDAY
    THURSDAY = calendar.THURSDAY
    FRIDAY = calendar.FRIDAY
    SATURDAY = calendar.SATURDAY


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
