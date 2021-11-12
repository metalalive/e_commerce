import enum
from datetime import datetime
from functools import partial

from sqlalchemy import Column, Integer, SmallInteger, Enum as sqlalchemy_enum, Float, String, Text, DateTime, ForeignKey, ForeignKeyConstraint
from sqlalchemy.orm import declarative_base, declarative_mixin, declared_attr, validates

from common.models.db import sqlalchemy_init_engine, sqlalchemy_db_conn, EmptyDataRowError
from common.models.enums.base import JsonFileChoicesMeta
from common.models.contact.sqlalchemy import EmailMixin, PhoneMixin, LocationMixin

from . import settings

sa_engine = sqlalchemy_init_engine(
        secrets_file_path=settings.SECRETS_FILE_PATH,
        secret_map=('order_service', 'backend_apps.databases.order_service'),
        base_folder='staff_portal', db_name=settings.DB_NAME,
        driver_label='mariadb+mariadbconnector',
        conn_args={}
    )

lowlvl_db_conn = partial(sqlalchemy_db_conn, engine=sa_engine)

# served as base of ORM mapped classes
Base = declarative_base()

class BuyerEmailMixin(EmailMixin):
    pass

class BuyerPhoneMixin(PhoneMixin):
    pass

class BuyerLocationMixin(LocationMixin):
    pass


class LoggingTimeMixin:
    # the time at which you created the item
    time_created = Column(DateTime, nullable=False, default=datetime.utcnow)
    last_updated = Column(DateTime, nullable=False, default=datetime.utcnow, onupdate=datetime.utcnow,)


# ORM mapped classes below
class TradeOrder(Base, LoggingTimeMixin):
    """
    This class represents order on both sides of trade behaviours
    """
    __tablename__ = 'trade_order'
    # describe legal contract / agreement that both of buyer and seller have to follow
    description = Column(Text)
    # each order (either purchase order or sale order) corresponds to only one seller
    # (supplier / vendor / whole-seller) and one buyer (end-customer / retailer)
    # Buyers should come from GenericUserProfile in user_management app  (offline user not accepted)
    buyer  = Column(Integer, nullable=False)
    # Sellers should come from StoreProfile in store app
    seller = Column(Integer, nullable=False)
    # TODO: add coupon / discount functionalities
    def _rand_gen_id(self):
        raise NotImplementedError()
    # TODO: randomly generate available ID for the new order
    id = Column(Integer, primary_key=True, autoincrement=False, default=_rand_gen_id)


class OrderInvoiceStateEnum(enum.Enum):
    PENDING = 1
    PROCESSING = 2
    CANCELLED = 3
    DONE = 4
    ARCHIVED = 5


class OrderInvoice(Base, LoggingTimeMixin):
    quota_material = settings._MatCodeOptions.MAX_NUM_ORDER_INVOICES
    __tablename__ = 'order_invoice'
    # Note one order can have multiple invoices,
    # e.g. In one purchase order, the vendor provides of purchased items in specific
    # time interval (e.g. once every week), and may invoice buyer each time on arrival
    # of the items, in such case, each invoice covers partial PO.
    order_id  = Column(Integer, ForeignKey('trade_order.id'), primary_key=True)
    seq  = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)
    # TODO, non-negative number check on tax field
    tax  = Column(Float, nullable=False, default=0.0) # invoice-level tax
    # TODO, check whether buyer left sufficient contact information, there should be
    # at least email or phone number in case the buyer gave wrong shipping address
    state = Column(sqlalchemy_enum(OrderInvoiceStateEnum), nullable=False)


@declarative_mixin
class OrderInvoiceReference:
    # buyer could optionally leave email to receive the invoice
    order_id    = Column(Integer, primary_key=True)
    invoice_seq = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)
    # this mixin requires composite foreign-key constraint which cannot be shared among
    # concrete classes using the mixin, however if you simply follow the SQLalchemy tutorial,
    # the same constraint will be referenced to each concrete class which uses the mixin.
    # In order to avoid conflict, use `declared_attr` decorator on the constraint and
    # __table_args__ , so each concrete class receives different constraint instance
    @declared_attr
    def pk_cfk_constraint(cls):
        return ForeignKeyConstraint(columns=['order_id', 'invoice_seq'],
            refcolumns=['order_invoice.order_id', 'order_invoice.seq'])

    @declared_attr
    def __table_args__(cls):
        return (cls.pk_cfk_constraint, {})


class OrderInvoiceBuyerEmail(Base, OrderInvoiceReference, BuyerEmailMixin):
    __tablename__ = 'order_invoice_buyer_email'

class OrderInvoiceBuyerPhone(Base, OrderInvoiceReference, BuyerPhoneMixin):
    __tablename__ = 'order_invoice_buyer_phone'

class OrderInvoiceBuyerLocation(Base, OrderInvoiceReference, BuyerLocationMixin):
    __tablename__ = 'order_invoice_buyer_location'


class UnitOfMeasurement(enum.Enum, metaclass=JsonFileChoicesMeta):
    filepath = 'common/data/unit_of_measurement.json'


class OrderLine(Base, OrderInvoiceReference):
    __tablename__ = 'order_line'
    seq  = Column(SmallInteger, primary_key=True, autoincrement=False)
    name = Column(String(128), nullable=False) # product name
    # accepted price for each item, without extra charge in attribute table
    amount = Column(Float, nullable=False)
    # tax for each item , TODO, positive number check on numeral fields
    tax = Column(Float, nullable=False)
    qty = Column(Float, nullable=False)
    uom = Column(sqlalchemy_enum(UnitOfMeasurement), nullable=False)


class OrderLineReference:
    order_id    = Column(Integer, primary_key=True)
    invoice_seq = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)
    item_seq    = Column(SmallInteger, primary_key=True, autoincrement=False)

    @declared_attr
    def pk_cfk_constraint(cls):
        return ForeignKeyConstraint(columns=['order_id', 'invoice_seq', 'item_seq'],
            refcolumns=['order_line.order_id', 'order_line.invoice_seq', 'order_line.seq'])

    @declared_attr
    def __table_args__(cls):
        return (cls.pk_cfk_constraint, {})


class OrderLineQuoteAttribute(Base, OrderLineReference):
    __tablename__ = 'order_line_quote_attribute'
    seq  = Column(SmallInteger, primary_key=True, autoincrement=False)
    attr_type = Column(String(64), nullable=False)
    value     = Column(String(64), nullable=False)
    data_type = Column(SmallInteger, nullable=False)
    extra_charge = Column(Float, nullable=False)


class OrderLineReturn(Base, OrderLineReference, LoggingTimeMixin):
    __tablename__ = 'order_line_return'
    # item-level return
    seq  = Column(SmallInteger, primary_key=True, autoincrement=False)
    # total amount of money as refund in each cancellation
    amount = Column(Float, nullable=False)
    untax = Column(Float, nullable=False)
    # quantity returned, TODO, validate, must not be larger than OrderLine.qty
    qty = Column(Float, nullable=False)
    reason = Column(Text)




# --------------------------------------------------
class OrderReceipt(Base, OrderInvoiceReference, LoggingTimeMixin):
    quota_material = settings._MatCodeOptions.MAX_NUM_ORDER_RECEIPTS
    __tablename__ = 'order_receipt'
    seq  = Column(SmallInteger, primary_key=True, autoincrement=False)

class OrderReceiptReference:
    order_id    = Column(Integer, primary_key=True)
    invoice_seq = Column(SmallInteger, primary_key=True, default=0, autoincrement=False)
    recpt_seq   = Column(SmallInteger, primary_key=True, autoincrement=False)

    @declared_attr
    def pk_cfk_constraint(cls):
        return ForeignKeyConstraint(columns=['order_id', 'invoice_seq', 'recpt_seq'],
            refcolumns=['order_receipt.order_id', 'order_receipt.invoice_seq', 'order_receipt.seq'])

    @declared_attr
    def __table_args__(cls):
        return (cls.pk_cfk_constraint, {})


# contact data may be different since billing address and shipping address may be different
class OrderReceiptBuyerEmail(Base, OrderReceiptReference, BuyerEmailMixin):
    __tablename__ = 'order_receipt_buyer_email'

class OrderReceiptBuyerPhone(Base, OrderReceiptReference, BuyerPhoneMixin):
    __tablename__ = 'order_receipt_buyer_phone'

class OrderReceiptShippingLocation(Base, OrderReceiptReference, BuyerLocationMixin):
    __tablename__ = 'order_receipt_shipping_location'
    # There may not be shipping address for virtual product
    # e.g. an online service, commercial computer software

class OrderReceiptDetail(Base, OrderReceiptReference):
    __tablename__ = 'order_receipt_detail'
    seq  = Column(SmallInteger, primary_key=True, autoincrement=False)
    # referential key to order line, only for fetching attributes of quote item
    ol_order_id    = Column(Integer)
    ol_invoice_seq = Column(SmallInteger, default=0, autoincrement=False)
    ol_item_seq    = Column(SmallInteger, autoincrement=False)
    ol_cfk_constraint = ForeignKeyConstraint(columns=['ol_order_id', 'ol_invoice_seq', 'ol_item_seq'],
            refcolumns=['order_line.order_id', 'order_line.invoice_seq', 'order_line.seq'])
    qty = Column(Float, nullable=False)

    @declared_attr
    def __table_args__(cls):
        return (cls.pk_cfk_constraint, cls.ol_cfk_constraint, {})


class OrderLineReview(Base, OrderLineReference, LoggingTimeMixin):
    __tablename__ = 'order_line_review'
    description = Column(Text)

class OrderLineReviewMedia(Base, OrderLineReference):
    __tablename__ = 'order_line_review_media'
    seq  = Column(SmallInteger, primary_key=True, autoincrement=False)
    media = Column(String(48), nullable=False) # media id reference to fileupload app


