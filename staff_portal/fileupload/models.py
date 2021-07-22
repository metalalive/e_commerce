from sqlalchemy import Column, Boolean, Integer, SmallInteger, String, BINARY, ForeignKey, ForeignKeyConstraint
from sqlalchemy.orm import declarative_base, relationship
from sqlalchemy.schema import PrimaryKeyConstraint

from .constants import ANONYMOUS_USER, ACCEPT_DUPLICATE, app_cfg
from .util import construct_resource_id, extract_resource_id, _determine_filestore_path, FileAttributeSetFactory, get_storage

Base = declarative_base()


class UploadedFile(Base):
    __tablename__ = 'uploadedfile'
    # SHA-1 hashed file content
    # storage location of the uplaoded file can be uniquely identified by the
    # combination of `hashed` field and `duplicate` field.
    hashed = Column(BINARY(length=20), primary_key=True)
    # if there is no duplicate file, the `duplicate` field always remains zero ,
    # otherwise the field is a unique value in the scope of a specific
    # file hash, the unique value can be either incremental number or random
    # number. Normally it is unlikely to have too many duplicates that share
    # a common hashed value
    duplicate = Column(SmallInteger, default=0, primary_key=True)
    # whether to discard the file , depending on whether any service references
    # the stored file.
    discard = Column(Boolean, default=True)
    size = Column(Integer, nullable=False) # size in bytes, TODO: positive number constraint
    uploader = Column(String(6), nullable=False) # profile ID from authorization server
    mimetype_toplvl = Column(String(11), nullable=False)
    mimetype_sub    = Column(String(9), nullable=False)
    # one-to-many relationship ,what if there are multiple foreign keys in the referenced table ?
    acl = relationship('FileAccessControlList', back_populates='resource')
    # if I need to scale the file servers in the future, extra column
    # `hostname` will be required in this table, and this service needs
    # a function to distribute uploaded files to different downstream servers
    __table_args__ = (PrimaryKeyConstraint(hashed, duplicate) , {})


class FileAccessControlList(Base):
    # multi-column foreign key (composite key) is declared only using
    # ForeignKeyConstraint() instead of adding ForeignKey() instance
    # to each individual column on the composite foreign-key (which causes
    # sqlalchemy.exc.AmbiguousForeignKeysError or sqlalchemy.exc.InvalidRequestError).
    file_hashed = Column(BINARY(length=20), primary_key=True) # ForeignKey('uploadedfile.hashed'),
    file_dup = Column(SmallInteger, primary_key=True) # ForeignKey('uploadedfile.duplicate'),
    file_cfk_constraint = ForeignKeyConstraint(columns=['file_hashed', 'file_dup'],
            refcolumns=['uploadedfile.hashed', 'uploadedfile.duplicate'])
    # composite primary key consists of :
    # * the composite foreign key declared above
    # * profile ID , which comes from authorization server
    prof_id = Column(String(6), primary_key=True, nullable=False)
    # access control flags
    rd_flg = Column(Boolean, nullable=False, default=False)
    rd_acl_flg = Column(Boolean, nullable=False, default=False)
    # to avoid ambiguity of join, each relationship() should be qualified by
    # instructing which foreign key column(s) should be considered
    resource   = relationship('UploadedFile', back_populates='acl',
            foreign_keys=[file_hashed, file_dup] )
    # for low-level schema
    __tablename__ = 'fileaccesscontrollist'
    __table_args__ = (file_cfk_constraint, {})


from functools import partial
from pymysql.constants.CLIENT import MULTI_STATEMENTS
# patch this before importing pydantic
from common.util.python import monkeypatch_typing_specialform
monkeypatch_typing_specialform()
from common.util.python.fastapi.settings import settings as fa_settings
from common.models.db import sqlalchemy_init_engine, sqlalchemy_insert, sqlalchemy_db_conn, EmptyDataRowError

storage_cfg = app_cfg['storage']

sa_engine = sqlalchemy_init_engine(
        secrets_file_path=fa_settings.secrets_file_path,
        secret_map=('file_upload_service', 'backend_apps.databases.file_upload_service'),
        base_folder='staff_portal',
        driver_label='mysql+pymysql',
        conn_args={'client_flag': MULTI_STATEMENTS}
    )

lowlvl_db_conn = partial(sqlalchemy_db_conn, engine=sa_engine)


@lowlvl_db_conn(enable_orm=True)
def check_upload_quota(uploader:str, limit_mb:int, newfile_sz_b:int, session=None):
    from sqlalchemy.sql import select as sa_select
    from sqlalchemy import func  as sa_func
    exceeded = True
    # TODO : check SQL injection
    stmt = sa_select(sa_func.sum(UploadedFile.size)).filter_by(
            uploader=uploader ).group_by(UploadedFile.uploader)
    qset = session.execute(stmt)
    row = qset.first() # run iterator and yield first row which hasn't been popped out
    limit_b = limit_mb * pow(2, 20)
    total_used_sz_b = newfile_sz_b
    if row:
        already_used_sz_b = row[0]
        total_used_sz_b += already_used_sz_b
    if limit_b > total_used_sz_b:
        exceeded = False
    return exceeded


@lowlvl_db_conn()
def save_upload_file(hashed:bytes, prof_id:str, _file, mimetype, dup_err, conn=None, non_file_types=None, num_retry=10):
    from sqlalchemy.exc import IntegrityError as SAIntegrityError
    from pymysql.constants.ER     import DUP_ENTRY
    non_file_types = non_file_types or []
    result = None
    mimetype = mimetype.split('/')
    data = {'hashed': hashed, 'duplicate':0, 'mimetype_toplvl':mimetype[0]
            , 'mimetype_sub':mimetype[1], 'uploader': prof_id}
    with conn.begin(): # transaction start
        try:
            for _ in range(num_retry):
                # if 2 threads invoke _get_duplicate_num() at the same time, they may
                # retrieve the same duplicate value, that implicitly causes DUP_ENTRY
                # exception or file-exists exception next time when caller attempts to
                # store the same file
                try:
                    result = _save_upload_file(data=data, conn=conn, _file=_file,
                             non_file_types=non_file_types)
                except FileExistsError as fe:
                    if ACCEPT_DUPLICATE:
                        data['duplicate'] = _get_duplicate_num(hashed, conn)
                    else:
                        raise dup_err
                except SAIntegrityError as sae:
                    err_args = sae.orig.args
                    if err_args[0] == DUP_ENTRY:
                        if ACCEPT_DUPLICATE:
                            data['duplicate'] = _get_duplicate_num(hashed, conn)
                        else:
                            raise dup_err
                    else:
                        raise # TODO log other types of error
                else: # succeed to persist file and insert record to database
                    break
            # end of for-loop
            assert result and hasattr(result, 'inserted_primary_key'), 'invalid result %s' % (result)
            assert hashed == result.inserted_primary_key[0], "stored hash value is inconsistent"
            data2 = {'file_hashed': hashed, 'prof_id':prof_id, 'rd_flg':True, 'rd_acl_flg':True}
            if ACCEPT_DUPLICATE:
                assert data['duplicate'] == result.inserted_primary_key[1], "stored duplicate value is inconsistent"
                data2['file_dup'] = data['duplicate']
            sqlalchemy_insert(model_cls_path='fileupload.models.FileAccessControlList', data=[data2], conn=conn)
        except:
            fs_storage = get_storage()
            store_path = _determine_filestore_path( hashed=data['hashed'], duplicate=data['duplicate'] )
            fs_storage.delete(store_path)
            raise
    # end of transaction
    return construct_resource_id(hashed, data['duplicate'])
## end of  save_upload_file()


def  _get_duplicate_num(hashed, conn):
    from sqlalchemy import desc as sa_desc
    from sqlalchemy.sql import select as sa_select
    from common.util.python import import_module_string
    model_cls_path='fileupload.models.UploadedFile'
    model_cls =  import_module_string(model_cls_path)
    table = model_cls.__table__
    s = sa_select(table.c.duplicate).where(table.c.hashed == hashed).order_by(sa_desc('duplicate')).limit(1)
    result = conn.execute(s)
    row = result.one() # or raise sqlalchemy.exc.NoResultFound, belongs to backend error
    return row[0] + 1


def  _save_upload_file(data, conn, _file, non_file_types):
    # firstly, persist file,
    fs_storage = get_storage()
    store_path = _determine_filestore_path( hashed=data['hashed'], duplicate=data['duplicate'] )
    # save() would raise FileExistsError once duplicate is found
    result = fs_storage.save(path=store_path, content=_file, async_flg=True, alt_path_autogen=False,
            chunk_sz=storage_cfg['chunk_size_bytes'],  non_file_types=non_file_types)
    data['size'] = result['size']
    # after file is stored successfully, insert the record to database
    result = sqlalchemy_insert(model_cls_path='fileupload.models.UploadedFile',
            data=[data], conn=conn)
    return result


@lowlvl_db_conn(enable_orm=False)
def get_file_attrs(resource_id, prof_ids=None, conn=None):
    from sqlalchemy import func as sa_func
    from sqlalchemy.sql import select as sa_select
    out_cls_kwargs = {}
    try:
        hashed, duplicate = extract_resource_id(resource_id, hash_alg='sha1')
        out_cls_kwargs.update({'hash_':hashed, 'duplicate':duplicate})
    except ValueError as e: # TODO, log error
        raise FileNotFoundError('invalid resource_id: %s' % resource_id)
    # `prof_id` is a varchar column, so it's necessary to convert each item to string
    # TODO: avoid SQL injection
    prof_ids = prof_ids or []
    prof_ids = list(map(str, prof_ids))
    if ANONYMOUS_USER not in prof_ids:
        prof_ids.append(ANONYMOUS_USER)
    # use raw DBAPI connection for multiple result sets
    dbapi_conn = conn.connection
    # which comes from low-level database driver, not standard SQLAlchemy class
    cursor = dbapi_conn.cursor()
    try:
        table  = UploadedFile.__table__
        table2 = FileAccessControlList.__table__
        hashed_hex = hashed.hex()
        # this application works with MariaDB, so I can apply HEX() expression function
        # on binary hash column to the raw SQL statements
        stmt  = sa_select(table.c.mimetype_toplvl, table.c.mimetype_sub, table.c.uploader
                ).where(sa_func.HEX(table.c.hashed) == hashed_hex, table.c.duplicate == duplicate)
        # prevent this function from fetching millions of access control records
        # related to only one file (by specifying profile ID)
        stmt2 = sa_select(table2.c.prof_id, table2.c.rd_flg, table2.c.rd_acl_flg).where(
                sa_func.HEX(table2.c.file_hashed) == hashed_hex, table2.c.file_dup == duplicate,
                table2.c.prof_id.in_(prof_ids))
        # cursor.execute() does NOT recognize SQLAlchemy expression object, which has to
        # be translated to raw SQL statement in advance
        sql_compile_fn = lambda s: str(s.compile(conn.engine, compile_kwargs={"literal_binds": True}))
        rawsqls = list(map(sql_compile_fn, [stmt, stmt2]))
        cursor.execute(';'.join(rawsqls)) # send 2 raw SQL statements in one round trip
        row = cursor.fetchone()
        # or raise TypeError if row is None (the resource doesn't exist)
        if not row:
            raise EmptyDataRowError('received empty row in result set 1')
        out_cls_kwargs['media_type'] = (row[0] , row[1])
        out_cls_kwargs['owner'] = row[2]
        cursor.nextset() # switch to next query set
        result = cursor.fetchall()
        out_cls_kwargs['acl'] = {}
        for prof_id, rd_flg, rd_acl_flg, in result:
            out_cls_kwargs['acl'][prof_id] = {'rd':bool(rd_flg), 'rd_acl':bool(rd_acl_flg)}
    except Exception as e: # TODO, log error
        raise
    finally:
        cursor.close()
    return  FileAttributeSetFactory(**out_cls_kwargs)
## end of  get_file_attrs()


