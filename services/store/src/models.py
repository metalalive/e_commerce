import logging
from typing import Dict, Tuple, List, Optional, Self, TYPE_CHECKING
from sqlalchemy import (
    Column,
    Boolean,
    JSON as JsonColType,
    SmallInteger,
    Enum as sqlalchemy_enum,
    String,
    Text,
    DateTime,
    Time,
    ForeignKey,
    delete as SqlAlDelete,
    select as SqlAlSelect,
    update as SqlAlUpdate,
    or_ as SqlAlOr,
    and_ as SqlAlAnd,
)
from sqlalchemy import func as sa_func, select as sa_select
from sqlalchemy.event import listens_for
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.mutable import MutableList
from sqlalchemy.orm import declarative_base, relationship, selectinload
from sqlalchemy.orm.attributes import set_committed_value
from sqlalchemy.inspection import inspect as sa_inspect
from sqlalchemy.dialects.mysql import INTEGER as MYSQL_INTEGER, BIGINT

from ecommerce_common.models.mixins import IdGapNumberFinder

from .dto import (
    EnumWeekDay,
    CountryCodeEnum,
    QuotaMatCode,
    EditProductDto,
    StoreCurrency,
    NewStoreProfileDto,
    EditExistingStoreProfileDto,
)

if TYPE_CHECKING:
    from .validation import (
        StoreStaffsReqBody,
        EditProductsReqBody,
    )

_logger = logging.getLogger(__name__)

Base = declarative_base()


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

    def extract_dup_id_from_error(self, error) -> int:
        raw_msg_chars = error.args[0].split()
        idx = raw_msg_chars.index("entry")
        dup_id = raw_msg_chars[idx + 1]
        dup_id = dup_id.strip("'")
        log_args = [
            "action",
            "extract-duplicate-id",
            "class",
            type(self).__name__,
            "dup-id",
            dup_id,
        ]
        _logger.debug(None, *log_args)
        return int(dup_id)


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
            new_items = tuple(filter(lambda obj: getattr(obj, attname) == target_id, objs))
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


# note that an organization (e.g. a company) can have several stores (either outlet or online)
class StoreProfile(Base, QuotaStatisticsMixin):
    quota_material = QuotaMatCode.MAX_NUM_STORES
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
    def from_req(cls, data: NewStoreProfileDto) -> Self:
        item = data.model_dump()  # convert to pure `dict` type
        item.pop("quota")
        item["emails"] = list(map(lambda d: StoreEmail(**d), item.get("emails", [])))
        item["phones"] = list(map(lambda d: StorePhone(**d), item.get("phones", [])))
        if item.get("location"):
            item["location"] = OutletLocation(**item["location"])
        return cls(**item)

    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="supervisor_id")

    @classmethod
    async def try_load(
        cls,
        session,
        store_id: int,
        eager_load_columns: Optional[List] = None,
    ) -> Optional[Self]:
        stmt = SqlAlSelect(cls).filter(cls.id == store_id)
        if eager_load_columns and len(eager_load_columns) > 0:
            cols = map(lambda v: selectinload(v), eager_load_columns)
            stmt = stmt.options(*cols)
        qset = await session.execute(stmt)
        result = qset.one_or_none()
        if result:
            return result[0]

    @classmethod
    async def bulk_insert(cls, objs: List[Self], session):
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

    def update(self, request: EditExistingStoreProfileDto):
        self.label = request.label
        self.active = request.active
        self.emails.clear()
        self.phones.clear()
        self.emails.extend(map(lambda d: StoreEmail(**d.model_dump()), request.emails))
        self.phones.extend(map(lambda d: StorePhone(**d.model_dump()), request.phones))
        if request.location:
            self.location = OutletLocation(**request.location.model_dump())
        else:
            self.location = None


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
    quota_material = QuotaMatCode.MAX_NUM_STAFF
    __tablename__ = "store_staff"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    # come from GenericUserProfile in user_management app
    staff_id = Column(MYSQL_INTEGER(unsigned=True), primary_key=True, autoincrement=False)
    store_applied = relationship("StoreProfile", back_populates="staff")

    @classmethod
    async def quota_stats(cls, objs, session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="store_id")

    @classmethod
    async def try_load(
        cls, session, store_id: int, reqdata: List["StoreStaffsReqBody"]
    ) -> List[Self]:
        staff_ids = [d.staff_id for d in reqdata]
        stmt = SqlAlSelect(cls).where(cls.store_id == store_id).where(cls.staff_id.in_(staff_ids))
        result = await session.execute(stmt)
        return [p[0] for p in result]

    @staticmethod
    def bulk_update(objs: List[Self], reqdata: List["StoreStaffsReqBody"]) -> List[int]:
        def _do_update(obj):
            newdata = filter(lambda d: d.staff_id == obj.staff_id, reqdata)
            newdata = next(newdata)
            assert newdata is not None
            obj.staff_id = newdata.staff_id
            obj.start_after = newdata.start_after
            obj.end_before = newdata.end_before
            return obj.staff_id

        return list(map(_do_update, objs))


class StoreEmail(Base, EmailMixin, QuotaStatisticsMixin):
    quota_material = QuotaMatCode.MAX_NUM_EMAILS
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
    quota_material = QuotaMatCode.MAX_NUM_PHONES
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


class StoreProductAvailable(Base, TimePeriodValidMixin, QuotaStatisticsMixin):
    quota_material = QuotaMatCode.MAX_NUM_PRODUCTS
    __tablename__ = "store_product_available"
    store_id = Column(
        MYSQL_INTEGER(unsigned=True),
        ForeignKey("store_profile.id", ondelete="CASCADE"),
        primary_key=True,
    )
    product_id = Column(BIGINT(unsigned=True), primary_key=True)
    base_price = Column(MYSQL_INTEGER(unsigned=True), nullable=False)
    store_applied = relationship("StoreProfile", back_populates="products")

    # currently number of attributes for each saleable item which requires extra amount
    # should not be large, also there should be small enough hard limit to avoid too much
    # settings cramming to a single column.
    attrs_charge = Column(MutableList.as_mutable(JsonColType), nullable=False)
    attrs_last_update = Column(DateTime, nullable=False)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    def __eq__(self, other: Self) -> bool:
        attrs_update = {(a["label_id"], a["value"], a["price"]) for a in other.attrs_charge}
        attrs_saved = {(a["label_id"], a["value"], a["price"]) for a in self.attrs_charge}
        return (
            (attrs_update == attrs_saved)
            and (self.store_id == other.store_id)
            and (self.product_id == other.product_id)
            and (self.base_price == other.base_price)
            and (self.start_after == other.start_after)
            and (self.end_before == other.end_before)
        )  # skip checking  `attrs_last_update`

    @classmethod
    async def quota_stats(cls, objs: List[Self], session, target_ids):
        return await super().quota_stats(objs, session, target_ids, attname="store_id")

    @classmethod
    def from_req(cls, store_id: int, req: "EditProductsReqBody") -> Self:
        return cls(
            store_id=store_id,
            product_id=req.product_id,
            base_price=req.base_price,
            start_after=req.start_after,
            end_before=req.end_before,
            attrs_last_update=req.attribute_lastupdate,
            attrs_charge=[a.model_dump() for a in req.attrs_charge],
        )

    @classmethod
    async def try_load(
        cls, session, store_id: int, reqdata: List["EditProductsReqBody"]
    ) -> List[Self]:
        product_ids = list(map(lambda d: d.product_id, reqdata))
        product_cond = cls.product_id.in_(product_ids)
        ## Don't use `saved_obj.products` generated by SQLAlchemy legacy Query API
        ## , instead I use `select` function to query relation fields
        final_cond = SqlAlAnd(cls.store_id == store_id, product_cond)
        stmt = SqlAlSelect(cls).where(final_cond)
        resultset = await session.execute(stmt)
        objs = [p[0] for p in resultset]  # tuple
        return objs

    @classmethod
    async def bulk_update(
        cls, session, objs: List[Self], reqs: List[EditProductDto]
    ) -> Dict[int, EditProductDto]:
        def _find_matched_item(obj: Self) -> Tuple[EditProductDto, Self]:
            def check_prod_id(d) -> bool:
                return d.product_id == obj.product_id

            req = next(filter(check_prod_id, reqs))
            assert req is not None
            return (req, obj)

        def _extract_update_item(req, obj_saved: Self) -> Optional[Dict]:
            obj_fromreq = cls.from_req(obj_saved.store_id, req)
            if obj_fromreq != obj_saved:
                arg = {
                    "store_id": obj_saved.store_id,
                    "product_id": obj_saved.product_id,
                    "base_price": req.base_price,
                    "start_after": req.start_after,
                    "end_before": req.end_before,
                    "attrs_charge": [a.model_dump() for a in req.attrs_charge],
                    "attrs_last_update": req.attribute_lastupdate,
                }
                return arg

        result1 = list(map(_find_matched_item, objs))
        updating_reqs = [a[0] for a in result1]
        result2 = [_extract_update_item(req, obj) for req, obj in result1]
        args = [a for a in result2 if a is not None]
        _ = await session.execute(SqlAlUpdate(cls), args)
        return {d.product_id: d for d in updating_reqs}

    @classmethod
    async def bulk_delete(cls, session, store_id: int, item_ids: List[int]) -> int:
        def _cond_fn(d):
            return SqlAlAnd(True, cls.product_id == d)

        pitem_cond = map(_cond_fn, item_ids)
        find_product_condition = SqlAlOr(*pitem_cond)
        stmt = SqlAlDelete(cls).where(cls.store_id == store_id).where(find_product_condition)
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
