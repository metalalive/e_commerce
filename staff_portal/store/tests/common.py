import random
import string
from datetime import time, datetime, timedelta

import pytest
from sqlalchemy.orm import Session
from mariadb.constants.CLIENT import MULTI_STATEMENTS

from store.settings import test as settings

from common.models.db import sqlalchemy_init_engine
from common.models.contact.sqlalchemy import CountryCodeEnum
from common.util.python import import_module_string
from tests.python.common.sqlalchemy import init_test_database, deinit_test_database, clean_test_data

from store.models import EnumWeekDay, SaleableTypeEnum, AppIdGapNumberFinder


metadata_objs = list(map(lambda path: import_module_string(dotted_path=path).metadata , settings.ORM_BASE_CLASSES))

@pytest.fixture(scope='session', autouse=True)
def db_engine_resource(request):
    # base setup / teardown for creating or deleting database and apply migration
    default_dbs_engine = sqlalchemy_init_engine(
            secrets_file_path=settings.SECRETS_FILE_PATH, base_folder='staff_portal',
            secret_map=(settings.DB_USER_ALIAS, 'backend_apps.databases.%s' % settings.DB_USER_ALIAS),
            driver_label=settings.DRIVER_LABEL
        ) # without specifying database name
    default_db_engine =  sqlalchemy_init_engine(
            secrets_file_path=settings.SECRETS_FILE_PATH, base_folder='staff_portal',
            secret_map=(settings.DB_USER_ALIAS, 'backend_apps.databases.%s' % settings.DB_USER_ALIAS),
            driver_label=settings.DRIVER_LABEL,  db_name=settings.DB_NAME,
            # TODO, for development and production environment, use configurable parameter
            # to optionally set multi_statement for the API endpoints that require to run
            # multiple SQL statements in one go.
            conn_args={'client_flag':MULTI_STATEMENTS}
        )
    keepdb = request.config.getoption('--keepdb', False)
    kwargs = {
        'dbs_engine':default_dbs_engine, 'db_engine':default_db_engine,
        'metadata_objs':metadata_objs, 'keepdb':keepdb
    }
    kwargs['createdb_sql'] = 'CREATE DATABASE IF NOT EXISTS `%s` DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin' % settings.DB_NAME
    kwargs['dropdb_sql'] = 'DROP DATABASE IF EXISTS `%s`' % settings.DB_NAME
    init_test_database(**kwargs)
    yield default_db_engine
    kwargs.pop('createdb_sql', None)
    kwargs.pop('metadata_objs', None)
    deinit_test_database(**kwargs)


@pytest.fixture
def session_for_test(db_engine_resource):
    with db_engine_resource.connect() as conn:
        try:
            with Session(bind=conn) as session:
                yield session
        finally: # TODO, optionally keep test data in database
            clean_test_data(conn, metadata_objs)
    ## not good, commits didn't actually write to database
    # with db_engine_resource.connect() as conn:
    #     with conn.begin() as transaction: # nested transaction starts
    #         try:
    #             with Session(bind=conn) as session:
    #                 yield session
    #         finally:
    #             # rollback the transaction made in previous session, so the session.commit()
    #             # can be called several times in the implementation code in which handles
    #             # data integrity 
    #             transaction.rollback()
    ### counterexample which may corrupt if session.commit() is invoked in test case
    # with Session(bind=db_engine_resource) as session:
    #     with session.begin() as transaction:
    #         try:
    #             yield session
    #         finally:
    #             transaction.rollback()


@pytest.fixture
def session_for_setup(db_engine_resource):
    with db_engine_resource.connect() as conn:
        with Session(bind=conn) as session:
            yield session


def _store_data_gen():
    idx = 2
    while True:
        new_data = {
                'active':random.choice([True,False]),
                'label':''.join(random.choices(string.ascii_letters, k=16)),
                'supervisor_id':idx,
                'id':None
            }
        yield new_data
        idx += 1

@pytest.fixture(scope='session')
def store_data():
    return _store_data_gen()

def _email_data_gen():
    while True:
        new_data = {
            'addr':'%s@%s.%s' % (
                ''.join(random.choices(string.ascii_letters, k=8)),
                ''.join(random.choices(string.ascii_letters, k=10)),
                ''.join(random.choices(string.ascii_letters, k=3))
            )
        }
        yield new_data

@pytest.fixture(scope='session')
def email_data():
    return _email_data_gen()


def _phone_data_gen():
    while True:
        new_data = {
            'country_code':str(random.randrange(1,999)),
            'line_number': str(random.randrange(0x10000000, 0xffffffff))
        }
        yield new_data

@pytest.fixture(scope='session')
def phone_data():
    return _phone_data_gen()

def _loc_data_gen():
    country_codes = [opt for opt in CountryCodeEnum]
    while True:
        new_data = {
            'country' : random.choice(country_codes) ,
            'locality': ''.join(random.choices(string.ascii_letters, k=40)) ,
            'street':   ''.join(random.choices(string.ascii_letters, k=40)) ,
            'detail':   ''.join(random.choices(string.ascii_letters, k=35)) ,
            'floor' :  random.randrange(-3, 10)
        }
        yield new_data

@pytest.fixture(scope='session')
def loc_data():
    return _loc_data_gen()

def _opendays_data_gen():
    idx = 0
    weekdays = [opt for opt in EnumWeekDay]
    while True:
        chosen_day = weekdays[idx]
        new_data = {
            'day': chosen_day ,
            'time_open':  time(hour=random.randrange(9,11),  minute=random.randrange(60)) ,
            'time_close': time(hour=random.randrange(17,22), minute=random.randrange(60)) ,
        }
        yield new_data
        idx = (idx + 1) % 7

@pytest.fixture(scope='session')
def opendays_data():
    return  _opendays_data_gen()


def _gen_time_period():
    start_minute = random.randrange(2, 100)
    day_length = random.randrange(365)
    start_after = datetime.utcnow() + timedelta(minutes=start_minute)
    end_before = start_after + timedelta(days=day_length)
    return start_after, end_before


def _staff_data_gen():
    staff_id = 3
    while True:
        start_after, end_before = _gen_time_period()
        new_data = {
            'staff_id': staff_id, 'start_after':start_after, 'end_before':end_before,
        }
        yield new_data
        staff_id += 1

@pytest.fixture(scope='session')
def staff_data():
    return  _staff_data_gen()


def _product_avail_data_gen():
    sale_types = [opt for opt in SaleableTypeEnum]
    while True:
        start_after, end_before = _gen_time_period()
        new_data = {
            'product_type': random.choice(sale_types),
            'product_id': random.randrange(1, AppIdGapNumberFinder.MAX_GAP_VALUE),
            'start_after':start_after,  'end_before':end_before,
        }
        yield new_data

@pytest.fixture(scope='session')
def product_avail_data():
    return  _product_avail_data_gen()


