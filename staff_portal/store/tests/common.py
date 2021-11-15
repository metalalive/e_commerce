import pytest
from sqlalchemy.orm import Session
from mariadb.constants.CLIENT import MULTI_STATEMENTS

from store.settings import test as settings

from common.models.db import sqlalchemy_init_engine
from common.util.python import import_module_string
from tests.python.common.sqlalchemy import init_test_database, deinit_test_database, clean_test_data

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
    init_test_database(**kwargs)
    yield default_db_engine
    kwargs.pop('createdb_sql', None)
    kwargs['dropdb_sql'] = 'DROP DATABASE IF EXISTS `%s`' % settings.DB_NAME
    deinit_test_database(**kwargs)


@pytest.fixture
def db_session(db_engine_resource):
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

