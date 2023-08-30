import os
import logging
from datetime import datetime , time as py_time
from functools import partial
from typing import Optional, List, Any, Union
from importlib import import_module

from mariadb.constants.CLIENT import MULTI_STATEMENTS
from fastapi import FastAPI, APIRouter, Header, Depends as FastapiDepends
from fastapi import HTTPException as FastApiHTTPException, status as FastApiHTTPstatus
from fastapi.security  import OAuth2AuthorizationCodeBearer
from pydantic import BaseModel as PydanticBaseModel, PositiveInt, constr, EmailStr, validator, ValidationError
from pydantic.errors import StrRegexError
from sqlalchemy import delete as SqlAlDelete
from sqlalchemy.orm import Session

from common.auth.fastapi import base_authentication, base_permission_check
from common.models.constants  import ROLE_ID_SUPERUSER
from common.models.db         import sqlalchemy_init_engine
from common.models.enums.base import AppCodeOptions, ActivationStatus
from common.models.contact.sqlalchemy import CountryCodeEnum
from common.util.python.messaging.rpc import RPCproxy

from .models import StoreProfile, StoreEmail, StorePhone, OutletLocation, StoreStaff, EnumWeekDay, HourOfOperation, SaleableTypeEnum, StoreProductAvailable

settings_module_path = os.getenv('APP_SETTINGS', 'store.settings.common')
settings = import_module(settings_module_path)

app_code = AppCodeOptions.store.value[0]

_logger = logging.getLogger(__name__)

shared_ctx = {}


async def app_shared_context_start(_app:FastAPI):
    from common.auth.keystore import create_keystore_helper
    from common.util.python import import_module_string
    shared_ctx['auth_app_rpc'] = RPCproxy(dst_app_name='user_management', src_app_name='store')
    shared_ctx['product_app_rpc'] = RPCproxy(dst_app_name='product', src_app_name='store')
    shared_ctx['auth_keystore'] = create_keystore_helper(cfg=settings.KEYSTORE, import_fn=import_module_string)
    return shared_ctx

async def app_shared_context_destroy(_app:FastAPI):
    rpcobj = shared_ctx.pop('auth_app_rpc')
    del rpcobj
    rpcobj = shared_ctx.pop('product_app_rpc')
    del rpcobj
    # note intepreter might not invoke `__del__()` for some cases
    # e.g. dependency cycle

router = APIRouter(
            prefix='', # could be API versioning e.g. /v0.0.1/* ,  /v2.0.1/*
            tags=['generic_store'] ,
            # TODO: dependencies are function executed before hitting the API endpoint
            # , (like router-level middleware for all downstream endpoints ?)
            dependencies=[],
            # the argument `lifespan` hasn't been integrated well in FastAPI an Starlette
            responses={
                FastApiHTTPstatus.HTTP_404_NOT_FOUND: {'description':'resource not found'},
                FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR: {'description':'internal server error'}
            }
        )


oauth2_scheme = OAuth2AuthorizationCodeBearer(
        authorizationUrl="no_auth_url",
        tokenUrl=settings.REFRESH_ACCESS_TOKEN_API_URL
    )

async def common_authentication(encoded_token:str=FastapiDepends(oauth2_scheme)):
    audience = ['store']
    return base_authentication(token=encoded_token, audience=audience,
            keystore=shared_ctx['auth_keystore'])


class Authorization:
    def __init__(self, app_code, perm_codes):
        self._app_code = app_code
        self._perm_codes = set(perm_codes)

    def __call__(self, user:dict=FastapiDepends(common_authentication)):
        # low-level permissions check
        error_obj = FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
            detail='Permission check failure',  headers={}
        )
        user = base_permission_check(user=user, app_code=self._app_code,
                required_perm_codes=self._perm_codes, error_obj=error_obj)
        return user

add_profile_authorization  = Authorization(app_code=app_code, perm_codes=['view_storeprofile', 'add_storeprofile'])
edit_profile_authorization = Authorization(app_code=app_code, perm_codes=['view_storeprofile', 'change_storeprofile'])
switch_supervisor_authorization = Authorization(app_code=app_code, perm_codes=['view_storeprofile', 'change_storeprofile'])
delete_profile_authorization = Authorization(app_code=app_code, perm_codes=['view_storeprofile', 'delete_storeprofile'])
edit_products_authorization = Authorization(app_code=app_code, perm_codes=['add_storeproductavailable', 'change_storeproductavailable', 'delete_storeproductavailable'])


class StoreEmailBody(PydanticBaseModel):
    class Config:
        orm_mode =True
    addr :EmailStr


class StorePhoneBody(PydanticBaseModel):
    class Config:
        orm_mode =True
    country_code : constr(regex=r"^\d{1,3}$")
    line_number : constr(regex=r"^\+?1?\d{7,15}$")

    def __init__(self, *args, **kwargs):
        custom_err_msg = {
            'country_code': "non-digit character detected, or length of digits doesn't meet requirement. It must contain only digit e.g. '91', '886' , from 1 digit up to 3 digits",
            'line_number': "non-digit character detected, or length of digits doesn't meet requirement. It must contain only digits e.g. '9990099', from 7 digits up to 15 digits",
        }
        try:
            super().__init__(*args, **kwargs)
        except ValidationError as ve:
            # custom error message according to different field
            for wrapper in ve.raw_errors:
                loc = wrapper._loc
                e = wrapper.exc
                if isinstance(e, StrRegexError):
                    e.msg_template = custom_err_msg[loc]
            raise


class OutletLocationBody(PydanticBaseModel):
    class Config:
        orm_mode =True
    country  :CountryCodeEnum
    locality :str
    street   :str
    detail   :str
    floor    :int


class NewStoreProfileReqBody(PydanticBaseModel):
    label : str
    supervisor_id : PositiveInt
    active : Optional[bool] = False
    emails : Optional[List[StoreEmailBody]] = []
    phones : Optional[List[StorePhoneBody]] = []
    location : Optional[OutletLocationBody] = None


def _get_supervisor_auth(prof_ids):
    reply_evt = shared_ctx['auth_app_rpc'].get_profile(ids=prof_ids, fields=['id', 'auth', 'quota'])
    if not reply_evt.finished:
        for _ in range(settings.NUM_RETRY_RPC_RESPONSE): # TODO, (1) async task (2) integration test
            reply_evt.refresh(retry=False, timeout=0.5, num_of_msgs_fetch=1)
            if reply_evt.finished:
                break
            else:
                pass
    rpc_response = reply_evt.result
    if rpc_response['status'] != reply_evt.status_opt.SUCCESS :
        raise FastApiHTTPException(
                status_code=FastApiHTTPstatus.HTTP_503_SERVICE_UNAVAILABLE,  headers={},
                detail={'app_code':[AppCodeOptions.user_management.value[0]]} )
    return rpc_response['result']


class NewStoreProfilesReqBody(PydanticBaseModel):
    __root__ : List[NewStoreProfileReqBody]

    @validator('__root__')
    def validate_list_items(cls, values):
        assert values and any(values), 'Empty request body Not Allowed'
        return values

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        req_prof_ids = list(set(map(lambda obj: obj.supervisor_id, self.__root__)))
        supervisor_verified = _get_supervisor_auth(req_prof_ids)
        quota_arrangement = self._get_quota_arrangement(supervisor_verified)
        self._storeemail_quota_check(quota_arrangement)
        self._storephone_quota_check(quota_arrangement)
        db_engine = _init_db_engine(conn_args={'client_flag':MULTI_STATEMENTS})
        self.metadata =  {'db_engine': db_engine}
        sa_new_stores = self._storeprofile_quota_check(db_engine, req_prof_ids, quota_arrangement)
        self.metadata['sa_new_stores'] = sa_new_stores

    def __del__(self):
        _metadata = getattr(self, 'metadata', {})
        db_engine = _metadata.get('db_engine')
        if db_engine:
            db_engine.dispose()

    def __setattr__(self, name, value):
        if name == 'metadata':
            # the attribute is for internal use, skip type checking
            self.__dict__[name] = value
        else:
            super().__setattr__(name, value)

    def _get_quota_arrangement(self, supervisor_verified):
        supervisor_verified = {item['id']:item for item in supervisor_verified}
        out = {}
        _fn  = lambda item: _get_quota_arrangement_helper(supervisor_verified, \
                    req_prof_id=item.supervisor_id, out=out)
        err_content = list(map(_fn, self.__root__))
        if any(err_content):
            raise FastApiHTTPException( detail=err_content,  headers={},
                    status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return out

    def _storeemail_quota_check(self, quota_arrangement):
        _contact_common_quota_check(self.__root__, quota_arrangement,
                label='emails', mat_model_cls=StoreEmail)

    def _storephone_quota_check(self, quota_arrangement):
        _contact_common_quota_check(self.__root__, quota_arrangement,
                label='phones', mat_model_cls=StorePhone)


    def _storeprofile_quota_check(self, db_engine, profile_ids, quota_arrangement):
        # quota check, for current user who adds these new items
        def _pydantic_to_sqlalchemy(item):
            item = item.dict()
            item['emails'] = list(map(lambda d:StoreEmail(**d), item.get('emails', [])))
            item['phones'] = list(map(lambda d:StorePhone(**d), item.get('phones', [])))
            if item.get('location'):
                item['location'] = OutletLocation(**item['location'])
            obj = StoreProfile(**item)
            return obj
        new_stores = list(map(_pydantic_to_sqlalchemy, self.__root__))
        with Session(bind=db_engine) as session:
            quota_chk_result = StoreProfile.quota_stats(new_stores, session=session, target_ids=profile_ids)
        def _inner_chk (item):
            err = {}
            num_existing_items = quota_chk_result[item.supervisor_id]['num_existing_items']
            num_new_items = quota_chk_result[item.supervisor_id]['num_new_items']
            curr_used = num_existing_items + num_new_items
            max_limit = quota_arrangement[item.supervisor_id][StoreProfile]
            if max_limit < curr_used:
                err['supervisor_id'] = item.supervisor_id
                err['store_profile'] = {'type':'limit-exceed', 'max_limit':max_limit,
                    'num_new_items':num_new_items, 'num_existing_items':num_existing_items}
            return err
        err_content = list(map(_inner_chk, self.__root__))
        if any(err_content):
            raise FastApiHTTPException( detail=err_content, headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
        return new_stores
## end of class NewStoreProfilesReqBody()


class ExistingStoreProfileReqBody(PydanticBaseModel):
    label  : str
    active : bool
    emails : Optional[List[StoreEmailBody]] = []
    phones : Optional[List[StorePhoneBody]] = []
    location : Optional[OutletLocationBody] = None


class StoreSupervisorReqBody(PydanticBaseModel):
    supervisor_id : PositiveInt # for new supervisor

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        req_prof_id = self.supervisor_id
        supervisor_verified = _get_supervisor_auth([req_prof_id])
        quota_arrangement = self._get_quota_arrangement(supervisor_verified, req_prof_id)
        db_engine = _init_db_engine()
        self.metadata =  {'db_engine': db_engine,}
        self._storeprofile_quota_check(db_engine, req_prof_id, quota_arrangement)

    def __del__(self):
        _metadata = getattr(self, 'metadata', {})
        db_engine = _metadata.get('db_engine')
        if db_engine:
            db_engine.dispose()

    def __setattr__(self, name, value):
        if name == 'metadata':
            # the attribute is for internal use, skip type checking
            self.__dict__[name] = value
        else:
            super().__setattr__(name, value)

    def _get_quota_arrangement(self, supervisor_verified, req_prof_id):
        supervisor_verified = {item['id']:item for item in supervisor_verified}
        out = {}
        err_detail = _get_quota_arrangement_helper(supervisor_verified, \
                req_prof_id=req_prof_id, out=out)
        if any(err_detail):
            raise FastApiHTTPException( detail=err_detail,  headers={},
                    status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return out

    def _storeprofile_quota_check(self, db_engine, prof_id, quota_arrangement):
        with Session(bind=db_engine) as session:
            quota_chk_result = StoreProfile.quota_stats([], session=session, target_ids=[prof_id])
        err = {}
        num_existing_items = quota_chk_result[prof_id]['num_existing_items']
        num_new_items = 1
        curr_used = num_existing_items + num_new_items
        max_limit = quota_arrangement[prof_id][StoreProfile]
        if max_limit < curr_used:
            err['supervisor_id'] = prof_id
            err['store_profile'] = {'type':'limit-exceed', 'max_limit':max_limit,
                'num_new_items':num_new_items, 'num_existing_items':num_existing_items}
        if any(err):
            raise FastApiHTTPException( detail=err, headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )



class StoreStaffReqBody(PydanticBaseModel):
    class Config:
        orm_mode =True
    staff_id : PositiveInt
    start_after : datetime
    end_before  : datetime

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if self.start_after > self.end_before:
            err_detail = {'code':'invalid_time_period'}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )


class StoreStaffsReqBody(PydanticBaseModel):
    __root__ : List[StoreStaffReqBody]

    @validator('__root__')
    def validate_list_items(cls, values):
        staff_ids = set(map(lambda obj:obj.staff_id , values))
        if len(staff_ids) != len(values):
            err_detail = {'code':'duplicate', 'field':['staff_id']}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return values

    def validate_staff(self, supervisor_id:int):
        staff_ids = list(map(lambda obj:obj.staff_id , self.__root__))
        reply_evt = shared_ctx['auth_app_rpc'].profile_descendant_validity(asc=supervisor_id, descs=staff_ids)
        if not reply_evt.finished:
            for _ in range(settings.NUM_RETRY_RPC_RESPONSE): # TODO, (1) async task (2) integration test
                reply_evt.refresh(retry=False, timeout=0.4, num_of_msgs_fetch=1)
                if reply_evt.finished:
                    break
                else:
                    pass
        rpc_response = reply_evt.result
        if rpc_response['status'] != reply_evt.status_opt.SUCCESS :
            raise FastApiHTTPException(
                    status_code=FastApiHTTPstatus.HTTP_503_SERVICE_UNAVAILABLE,  headers={},
                    detail={'app_code':[AppCodeOptions.user_management.value[0]]}
                )
        validated_staff_ids = rpc_response['result']
        diff = set(staff_ids) - set(validated_staff_ids)
        if any(diff):
            err_detail = {'code':'invalid_descendant', 'supervisor_id':supervisor_id , 'staff_ids': list(diff)}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return validated_staff_ids


class BusinessHoursDayReqBody(PydanticBaseModel):
    day  : EnumWeekDay
    time_open  : py_time
    time_close : py_time
    class Config:
        orm_mode =True

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if self.time_open > self.time_close:
            err_detail = {'code':'invalid_time_period'}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )

class BusinessHoursDaysReqBody(PydanticBaseModel):
    __root__ : List[BusinessHoursDayReqBody]
    class Config:
        orm_mode =True

    @validator('__root__')
    def validate_list_items(cls, values):
        days = set(map(lambda obj:obj.day , values))
        if len(days) != len(values):
            err_detail = {'code':'duplicate', 'field':['day']}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return values


class EditProductReqBody(PydanticBaseModel):
    product_type : SaleableTypeEnum
    product_id   : PositiveInt
    price       : PositiveInt
    start_after : datetime
    end_before  : datetime
    class Config:
        orm_mode =True

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if self.start_after > self.end_before:
            err_detail = {'code':'invalid_time_period'}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )


class EditProductsReqBody(PydanticBaseModel):
    __root__ : List[EditProductReqBody]
    class Config:
        orm_mode =True

    @validator('__root__')
    def validate_list_items(cls, values):
        prod_ids = set(map(lambda obj:(obj.product_type.value , obj.product_id) , values))
        if len(prod_ids) != len(values):
            err_detail = {'code':'duplicate', 'field':['product_type', 'product_id']}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return values

    def validate_products(self, staff_id:int):
        filtered = filter(lambda obj:obj.product_type == SaleableTypeEnum.ITEM , self.__root__)
        item_ids = list(map(lambda obj:obj.product_id , filtered))
        filtered = filter(lambda obj:obj.product_type == SaleableTypeEnum.PACKAGE , self.__root__)
        pkg_ids  = list(map(lambda obj:obj.product_id , filtered))
        fields_present = ['id',]
        reply_evt = shared_ctx['product_app_rpc'].get_product(item_ids=item_ids, pkg_ids=pkg_ids,
                profile=staff_id, item_fields=fields_present, pkg_fields=fields_present)
        if not reply_evt.finished:
            for _ in range(settings.NUM_RETRY_RPC_RESPONSE): # TODO, (1) async task (2) integration test
                reply_evt.refresh(retry=False, timeout=0.4, num_of_msgs_fetch=1)
                if reply_evt.finished:
                    break
                else:
                    pass
        rpc_response = reply_evt.result
        if rpc_response['status'] != reply_evt.status_opt.SUCCESS :
            raise FastApiHTTPException(
                    status_code=FastApiHTTPstatus.HTTP_503_SERVICE_UNAVAILABLE,  headers={},
                    detail={'app_code':[AppCodeOptions.product.value[0]]}
                )
        validated_data = rpc_response['result']
        validated_item_ids = set(map(lambda d:d['id'], validated_data['item']))
        validated_pkg_ids  = set(map(lambda d:d['id'], validated_data['pkg']))
        diff_item = set(item_ids) - validated_item_ids
        diff_pkg  = set(pkg_ids)  - validated_pkg_ids
        err_detail = {'code':'invalid', 'field':[]}
        if any(diff_item):
            diff_item = map(lambda v:{'product_type':SaleableTypeEnum.ITEM.value, 'product_id':v}, diff_item)
            err_detail['field'].extend( list(diff_item) )
        if any(diff_pkg):
            diff_pkg = map(lambda v:{'product_type':SaleableTypeEnum.PACKAGE.value, 'product_id':v}, diff_pkg)
            err_detail['field'].extend( list(diff_pkg) )
        if any(err_detail['field']):
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )


class DiscardProductReqBody(PydanticBaseModel):
    product_type : SaleableTypeEnum
    product_id   : PositiveInt

class DiscardProductsReqBody(PydanticBaseModel):
    __root__ : List[DiscardProductReqBody]
    class Config:
        orm_mode =True


def _get_quota_arrangement_helper(supervisor_verified:dict, req_prof_id:int, out:dict):
    err_detail = []
    item = supervisor_verified.get(req_prof_id, None)
    if item:
        auth_status = item.get('auth', ActivationStatus.ACCOUNT_NON_EXISTENT.value)
        if auth_status != ActivationStatus.ACCOUNT_ACTIVATED.value:
            err_detail.append('unable to login')
        if out.get(req_prof_id):
            log_args = ['action', 'duplicate-quota', 'req_prof_id', str(req_prof_id)]
            _logger.warning(None, *log_args)
        else:
            quota_material_models = (StoreProfile, StoreEmail, StorePhone)
            out[req_prof_id] = {}
            quota = item.get('quota', [])
            filter_fn = lambda d, model_cls: d['mat_code'] == model_cls.quota_material.value
            for model_cls in quota_material_models:
                bound_filter_fn = partial(filter_fn, model_cls=model_cls)
                filtered = tuple(filter(bound_filter_fn, quota))
                maxnum = filtered[0]['maxnum'] if any(filtered) else 0
                out[req_prof_id][model_cls] = maxnum
    else:
        err_detail.append('non-existent user profile')
    err_detail = {'supervisor_id':err_detail} if any(err_detail) else {}
    return  err_detail

def _contact_common_quota_check(req, quota_arrangement:dict, label:str,
        mat_model_cls:Union[StoreEmail, StorePhone] ):
    def _inner_chk (item):
        err = {}
        num_new_items = len(getattr(item, label))
        max_limit = quota_arrangement[item.supervisor_id][mat_model_cls]
        if max_limit < num_new_items:
            err['supervisor_id'] = item.supervisor_id
            err[label] = {'type':'limit-exceed', 'max_limit':max_limit,
                    'num_new_items':num_new_items}
        return err
    err_content = list(map(_inner_chk, req))
    if any(err_content):
        raise FastApiHTTPException( detail=err_content,  headers={},
                status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )


class StoreProfileResponseBody(PydanticBaseModel):
    id : PositiveInt
    supervisor_id :  PositiveInt


class StoreProfileReadResponseBody(PydanticBaseModel):
    class Config:
        orm_mode =True
    label  : str
    active : bool
    supervisor_id :  PositiveInt
    emails : Optional[List[StoreEmailBody]] = []
    phones : Optional[List[StorePhoneBody]] = []
    location : Optional[OutletLocationBody] = None
    staff     : Optional[List[StoreStaffReqBody]] = []
    open_days : Optional[List[BusinessHoursDayReqBody]] = []


def _init_db_engine(conn_args:Optional[dict]=None):
    """ TODO
      - for development and production environment, use configurable parameter
        to optionally set multi_statement for the API endpoints that require to run
        multiple SQL statements in one go.
      - the engine is the most efficient when created at module-level of application
        , not per function or per request, modify the implementation in this app.
    """ 
    kwargs = {
        'secrets_file_path':settings.SECRETS_FILE_PATH, 'base_folder':'staff_portal',
        'secret_map':(settings.DB_USER_ALIAS, 'backend_apps.databases.%s' % settings.DB_USER_ALIAS),
        'driver_label':settings.DRIVER_LABEL, 'db_name':settings.DB_NAME,
    }
    if conn_args:
        kwargs['conn_args'] = conn_args
    return sqlalchemy_init_engine(**kwargs)


def _store_existence_validity(session, store_id:PositiveInt):
    query = session.query(StoreProfile).filter(StoreProfile.id == store_id)
    saved_obj = query.first()
    if not saved_obj:
        raise FastApiHTTPException( detail={'code':'not_exist'},  headers={},
                status_code=FastApiHTTPstatus.HTTP_404_NOT_FOUND )
    return saved_obj


def _store_supervisor_validity(session, store_id:PositiveInt, usr_auth:dict):
    saved_obj = _store_existence_validity(session, store_id)
    if usr_auth['priv_status'] != ROLE_ID_SUPERUSER and saved_obj.supervisor_id != usr_auth['profile']:
        raise FastApiHTTPException( detail='Not allowed to edit the store profile',  headers={},
                status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
    return saved_obj


def _store_staff_validity(session, store_id:PositiveInt, usr_auth:dict):
    saved_obj = _store_existence_validity(session, store_id)
    valid_staff_ids = list(map(lambda o:o.staff_id, saved_obj.staff))
    valid_staff_ids.append(saved_obj.supervisor_id)
    if usr_auth['profile'] not in valid_staff_ids:
        raise FastApiHTTPException( detail='Not allowed to edit the store products',  headers={},
                status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
    return saved_obj


@router.post('/profiles', status_code=FastApiHTTPstatus.HTTP_201_CREATED, response_model=List[StoreProfileResponseBody])
def add_profiles(request:NewStoreProfilesReqBody, user:dict=FastapiDepends(add_profile_authorization)):
    db_engine = request.metadata['db_engine']
    sa_new_stores = request.metadata['sa_new_stores']
    with db_engine.connect() as conn:
        with Session(bind=conn) as session:
            StoreProfile.bulk_insert(objs=sa_new_stores, session=session)
            _fn = lambda obj: StoreProfileResponseBody(id=obj.id, supervisor_id=obj.supervisor_id)
            resp_data = list(map(_fn, sa_new_stores))
    return resp_data
## def add_profiles()


@router.patch('/profile/{store_id}',)
def edit_profile(store_id:PositiveInt, request:ExistingStoreProfileReqBody, user:dict=FastapiDepends(edit_profile_authorization)):
    # part of authorization has to be handled at here because it requires all these arguments
    quota_arrangement = dict(map(lambda d:(d['mat_code'], d['maxnum']), user['quota']))
    num_new_items = len(request.emails)
    max_limit = quota_arrangement.get(StoreEmail.quota_material.value, 0)
    if max_limit < num_new_items:
        err_msg = 'Limit exceeds, num_new_items:%s, max_limit:%s' % (num_new_items, max_limit)
        raise FastApiHTTPException( detail={'emails':[err_msg]},  headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
    num_new_items = len(request.phones)
    max_limit = quota_arrangement.get(StorePhone.quota_material.value, 0)
    if max_limit < num_new_items:
        err_msg = 'Limit exceeds, num_new_items:%s, max_limit:%s' % (num_new_items, max_limit)
        raise FastApiHTTPException( detail={'phones':[err_msg]},  headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
    # TODO, figure out better way to authorize with database connection
    db_engine = _init_db_engine()
    try:
        with Session(bind=db_engine) as session:
            saved_obj = _store_supervisor_validity(session, store_id, usr_auth=user)
            # perform update
            saved_obj.label  = request.label
            saved_obj.active = request.active
            saved_obj.emails.clear()
            saved_obj.phones.clear()
            saved_obj.emails.extend( list(map(lambda d:StoreEmail(**d.dict()), request.emails)) )
            saved_obj.phones.extend( list(map(lambda d:StorePhone(**d.dict()), request.phones)) )
            if request.location:
                saved_obj.location = OutletLocation(**request.location.dict())
            else:
                saved_obj.location = None
            session.commit()
    finally:
        db_engine.dispose()
    return None



@router.patch('/profile/{store_id}/supervisor',)
def switch_supervisor(store_id:PositiveInt, request:StoreSupervisorReqBody, user:dict=FastapiDepends(switch_supervisor_authorization)):
    db_engine = request.metadata['db_engine']
    with Session(bind=db_engine) as session:
        saved_obj = _store_supervisor_validity(session, store_id, usr_auth=user)
        saved_obj.supervisor_id = request.supervisor_id
        session.commit()
    return None

@router.delete('/profiles', status_code=FastApiHTTPstatus.HTTP_204_NO_CONTENT)
def delete_profile(ids:str, user:dict=FastapiDepends(delete_profile_authorization)):
    try:
        ids = list(map(int, ids.split(',')))
        if len(ids) == 0:
            raise FastApiHTTPException( detail={'ids':'empty'},  headers={},
                    status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
    except ValueError as e:
        raise FastApiHTTPException( detail={'ids':'invalid-id'},  headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
    db_engine = _init_db_engine()
    with Session(bind=db_engine) as _session:
        stmt = SqlAlDelete(StoreProfile).where(StoreProfile.id.in_(ids))
        result = _session.execute(stmt) # TODO, consider soft-delete
        _session.commit()
    # Note python does not have `scope` concept, I can access the variables
    # `result` and `ids` declared above.
    if result.rowcount == 0:
        raise FastApiHTTPException( detail={},  headers={},
                status_code=FastApiHTTPstatus.HTTP_410_GONE )
    return None


@router.patch('/profile/{store_id}/staff',)
def edit_staff(store_id:PositiveInt, request:StoreStaffsReqBody, user:dict=FastapiDepends(edit_profile_authorization)):
    request.validate_staff(supervisor_id=user['profile'])
    db_engine = _init_db_engine()
    try:
        with Session(bind=db_engine) as session:
            saved_obj = _store_supervisor_validity(session, store_id, usr_auth=user)
            new_staff = list(map(lambda d:StoreStaff(**d.dict()), request.__root__ ))
            saved_obj.staff.clear()
            saved_obj.staff.extend(new_staff)
            session.commit()
    finally:
        db_engine.dispose()
    return None


@router.patch('/profile/{store_id}/business_hours',)
def edit_hours_operation(store_id:PositiveInt, request:BusinessHoursDaysReqBody, \
        user:dict=FastapiDepends(edit_profile_authorization)):
    db_engine = _init_db_engine()
    try:
        with Session(bind=db_engine) as session:
            saved_obj = _store_supervisor_validity(session, store_id, usr_auth=user)
            new_time = list(map(lambda d:HourOfOperation(**d.dict()), request.__root__))
            saved_obj.open_days.clear()
            saved_obj.open_days.extend(new_time)
            session.commit()
    finally:
        db_engine.dispose()


@router.patch('/profile/{store_id}/products',)
def edit_products_available(store_id:PositiveInt, request: EditProductsReqBody, \
        user:dict=FastapiDepends(edit_products_authorization)):
    request.validate_products(staff_id=user['profile'])
    db_engine = _init_db_engine()
    try: # TODO, modify this endpoint for editing-only operation
        with Session(bind=db_engine) as session:
            saved_obj = _store_staff_validity(session, store_id, usr_auth=user)
            new_prod_objs = list(map(lambda d: StoreProductAvailable(**d.dict()), request.__root__ ))
            saved_obj.products.clear()
            saved_obj.products.extend(new_prod_objs)
            session.commit()
    finally:
        db_engine.dispose()


# TODO , complete implementation
@router.delete('/profile/{store_id}/products', status_code=FastApiHTTPstatus.HTTP_204_NO_CONTENT)
def discard_store_products(store_id:PositiveInt, request: DiscardProductsReqBody, \
        user:dict=FastapiDepends(edit_products_authorization)):
    raise FastApiHTTPException( detail='',  headers={},
                status_code=FastApiHTTPstatus.HTTP_501_NOT_IMPLEMENTED )


@router.get('/profile/{store_id}/products', response_model=EditProductsReqBody)
def read_profile_products(store_id:PositiveInt, user:dict=FastapiDepends(common_authentication)):
    db_engine = _init_db_engine()
    try: # TODO, figure out how to handle large dataset, pagination or other techniques
        with Session(bind=db_engine) as session:
            saved_obj = _store_staff_validity(session, store_id, usr_auth=user)
            response = EditProductsReqBody.from_orm(saved_obj.products)
    finally:
        db_engine.dispose()
    return response


@router.get('/profile/{store_id}', response_model=StoreProfileReadResponseBody)
def read_profile(store_id:PositiveInt, user:dict=FastapiDepends(common_authentication)):
    db_engine = _init_db_engine()
    try:
        with Session(bind=db_engine) as session:
            saved_obj = _store_staff_validity(session, store_id, usr_auth=user)
            response = StoreProfileReadResponseBody.from_orm(saved_obj)
    finally:
        db_engine.dispose()
    return response

