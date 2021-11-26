import os
from datetime import datetime , time as py_time
from functools import partial
from typing import Optional, List, Any
from importlib import import_module

from mariadb.constants.CLIENT import MULTI_STATEMENTS
from fastapi import APIRouter, Header, Depends as FastapiDepends
from fastapi import HTTPException as FastApiHTTPException, status as FastApiHTTPstatus
from fastapi.security  import OAuth2AuthorizationCodeBearer
from pydantic import BaseModel as PydanticBaseModel, PositiveInt, constr, EmailStr, validator, ValidationError
from pydantic.errors import StrRegexError
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

router = APIRouter(
            prefix='', # could be /store/* /file/* ... etc
            tags=['generic_store'] ,
            # TODO: dependencies are function executed before hitting the API endpoint
            # , (like router-level middleware for all downstream endpoints ?)
            dependencies=[],
            responses={
                FastApiHTTPstatus.HTTP_404_NOT_FOUND: {'description':'resource not found'},
                FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR: {'description':'internal server error'}
            }
        )

auth_app_rpc = RPCproxy(dst_app_name='user_management', src_app_name='store')
product_app_rpc = RPCproxy(dst_app_name='product', src_app_name='store')

oauth2_scheme = OAuth2AuthorizationCodeBearer(
        authorizationUrl="no_auth_url",
        tokenUrl=settings.REFRESH_ACCESS_TOKEN_API_URL
    )

async def common_authentication(encoded_token:str=FastapiDepends(oauth2_scheme)):
    audience = ['store']
    error_obj = FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_401_UNAUTHORIZED,
            detail='authentication failure',
            headers={'www-Authenticate': 'Bearer'}
        )
    return base_authentication(token=encoded_token, audience=audience,
            ks_cfg=settings.KEYSTORE, error_obj=error_obj)


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
    addr :EmailStr


class StorePhoneBody(PydanticBaseModel):
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
    reply_evt = auth_app_rpc.get_profile(ids=prof_ids, field_names=['id', 'auth', 'quota'])
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
                detail={'app_code':[AppCodeOptions.user_management.value[0]]}
            )
    return rpc_response['result']


class NewStoreProfilesReqBody(PydanticBaseModel):
    __root__ : List[NewStoreProfileReqBody]

    @validator('__root__')
    def validate_list_items(cls, values):
        assert values and any(values), 'Empty request body Not Allowed'
        return values

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        prof_ids = list(set(map(lambda obj: obj.supervisor_id, self.__root__)))
        supervisor_data = _get_supervisor_auth(prof_ids)
        quota_arrangement = self._get_quota_arrangement(supervisor_data, prof_ids)
        self._storeemail_quota_check(quota_arrangement)
        self._storephone_quota_check(quota_arrangement)
        db_engine = _init_db_engine(conn_args={'client_flag':MULTI_STATEMENTS})
        self.metadata =  {'db_engine': db_engine}
        sa_new_stores =self._storeprofile_quota_check(db_engine, prof_ids, quota_arrangement)
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

    def _get_quota_arrangement(self, supervisor_data, profile_ids):
        # identity check, are they existing users ? through inter-apps message queue
        err_content = []
        quota_material_models = (StoreProfile, StoreEmail, StorePhone)
        supervisor_data = {item['id']:item for item in supervisor_data}
        quota_arrangement = {}
        for item in self.__root__:
            err_detail = []
            prof_id = item.supervisor_id
            err_detail = _get_quota_arrangement_helper(supervisor_data, prof_id, quota_arrangement)
            err_content.append(err_detail)
        if any(err_content):
            raise FastApiHTTPException( detail=err_content,  headers={},
                    status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return quota_arrangement

    def _storeemail_quota_check(self, quota_arrangement):
        err_content = []
        for item in self.__root__:
            err_item = {}
            prof_id = item.supervisor_id
            num_new_items = len(item.emails)
            max_limit = quota_arrangement[prof_id][StoreEmail]
            if max_limit < num_new_items:
                err_msg = 'Limit exceeds, num_new_items:%s, max_limit:%s' % (num_new_items, max_limit)
                err_item['emails'] = [err_msg]
            err_content.append(err_item)
        if any(err_content):
            raise FastApiHTTPException( detail=err_content,  headers={},
                    status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )

    def _storephone_quota_check(self, quota_arrangement):
        err_content = []
        for item in self.__root__:
            err_item = {}
            prof_id = item.supervisor_id
            num_new_items = len(item.phones)
            max_limit = quota_arrangement[prof_id][StorePhone]
            if max_limit < num_new_items:
                err_msg = 'Limit exceeds, num_new_items:%s, max_limit:%s' % (num_new_items, max_limit)
                err_item['phones'] = [err_msg]
            err_content.append(err_item)
        if any(err_content):
            raise FastApiHTTPException( detail=err_content,  headers={},
                    status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )


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
        err_content = []
        new_stores = list(map(_pydantic_to_sqlalchemy, self.__root__))
        with Session(bind=db_engine) as session:
            quota_chk_result = StoreProfile.quota_stats(new_stores, session=session, target_ids=profile_ids)
        for  item in self.__root__:
            err_item = {}
            prof_id = item.supervisor_id
            num_existing_items = quota_chk_result[prof_id]['num_existing_items']
            num_new_items = quota_chk_result[prof_id]['num_new_items']
            curr_used = num_existing_items + num_new_items
            max_limit = quota_arrangement[prof_id][StoreProfile]
            if max_limit < curr_used:
                err_msg = 'Limit exceeds, num_existing_items:%s, num_new_items:%s, max_limit:%s' % (num_existing_items, num_new_items, max_limit)
                err_item['supervisor_id'] = [err_msg]
            err_content.append(err_item)
        if any(err_content):
            raise FastApiHTTPException( detail=err_content, headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
        return new_stores
## end of class NewStoreProfilesReqBody()


class DeleteStoreProfilesReqBody(PydanticBaseModel):
    ids : List[PositiveInt]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        db_engine = _init_db_engine()
        session = Session(bind=db_engine)
        self.metadata =  {'db_engine': db_engine, 'session':session}
        query = session.query(StoreProfile).filter(StoreProfile.id.in_(self.ids))
        objs = query.all()
        actual_ids = set(map(lambda obj:obj.id , objs))
        missing = set(self.ids) - actual_ids
        if missing:
            raise FastApiHTTPException( detail='Some of stores do not exist, ID = %s' % missing,
                    headers={}, status_code=FastApiHTTPstatus.HTTP_410_GONE )
        self.metadata['deleting_objs'] = objs

    def __del__(self):
        _metadata = getattr(self, 'metadata', {})
        session = _metadata.get('session')
        if session:
            session.close()
        db_engine = _metadata.get('db_engine')
        if db_engine:
            db_engine.dispose()

    def __setattr__(self, name, value):
        if name == 'metadata':
            # the attribute is for internal use, skip type checking
            self.__dict__[name] = value
        else:
            super().__setattr__(name, value)


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
        prof_id = self.supervisor_id
        supervisor_data = _get_supervisor_auth([prof_id])
        quota_arrangement = self._get_quota_arrangement(supervisor_data, prof_id)
        db_engine = _init_db_engine()
        self.metadata =  {'db_engine': db_engine,}
        self._storeprofile_quota_check(db_engine, prof_id, quota_arrangement)

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

    def _get_quota_arrangement(self, supervisor_data, prof_id):
        # identity check, are they existing users ? through inter-apps message queue
        supervisor_data = {item['id']:item for item in supervisor_data}
        quota_arrangement = {}
        err_detail = _get_quota_arrangement_helper(supervisor_data, prof_id, quota_arrangement)
        if any(err_detail):
            raise FastApiHTTPException( detail=err_detail,  headers={},
                    status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return quota_arrangement

    def _storeprofile_quota_check(self, db_engine, prof_id, quota_arrangement):
        with Session(bind=db_engine) as session:
            quota_chk_result = StoreProfile.quota_stats([], session=session, target_ids=[prof_id])
        err_item = {}
        num_existing_items = quota_chk_result[prof_id]['num_existing_items']
        num_new_items = 1
        curr_used = num_existing_items + num_new_items
        max_limit = quota_arrangement[prof_id][StoreProfile]
        if max_limit < curr_used:
            err_msg = 'Limit exceeds, num_existing_items:%s, num_new_items:%s, max_limit:%s' % (num_existing_items, num_new_items, max_limit)
            err_item['supervisor_id'] = [err_msg]
        if any(err_item):
            raise FastApiHTTPException( detail=err_item, headers={}, status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )



class StoreStaffReqBody(PydanticBaseModel):
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
        reply_evt = auth_app_rpc.profile_descendant_validity(asc=supervisor_id, descs=staff_ids)
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

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if self.time_open > self.time_close:
            err_detail = {'code':'invalid_time_period'}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )

class BusinessHoursDaysReqBody(PydanticBaseModel):
    __root__ : List[BusinessHoursDayReqBody]

    @validator('__root__')
    def validate_list_items(cls, values):
        days = set(map(lambda obj:obj.day , values))
        if len(days) != len(values):
            err_detail = {'code':'duplicate', 'field':['day']}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
        return values


class AvailProductReqBody(PydanticBaseModel):
    product_type : SaleableTypeEnum
    product_id   : PositiveInt
    start_after : datetime
    end_before  : datetime

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if self.start_after > self.end_before:
            err_detail = {'code':'invalid_time_period'}
            raise FastApiHTTPException( detail=err_detail, headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )


class AvailProductsReqBody(PydanticBaseModel):
    __root__ : List[AvailProductReqBody]

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
        reply_evt = product_app_rpc.get_product(item_ids=item_ids, pkg_ids=pkg_ids, profile=staff_id,
                item_fields=fields_present, pkg_fields=fields_present)
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


def _get_quota_arrangement_helper(supervisor_data, prof_id, quota_arrangement):
    quota_material_models = (StoreProfile, StoreEmail, StorePhone)
    err_detail = []
    item = supervisor_data.get(prof_id, {})
    if not item:
        err_detail.append('non-existent user profile')
    auth_status = item.get('auth', None)
    if auth_status != ActivationStatus.ACCOUNT_ACTIVATED.value:
        err_detail.append('unable to login')
    if quota_arrangement.get(prof_id, None) is None:
        quota_arrangement[prof_id] = {}
        quota = item.get('quota', [])
        filter_fn = lambda d, model_cls: d['mat_code'] == model_cls.quota_material.value
        for model_cls in quota_material_models:
            bound_filter_fn = partial(filter_fn, model_cls=model_cls)
            filtered = tuple(filter(bound_filter_fn, quota))
            maxnum = filtered[0]['maxnum'] if any(filtered) else 0
            quota_arrangement[prof_id][model_cls] = maxnum
    err_detail = {'supervisor_id':err_detail} if any(err_detail) else {}
    return  err_detail


class StoreProfileResponseBody(PydanticBaseModel):
    id : PositiveInt
    supervisor_id :  PositiveInt


def _init_db_engine(conn_args:Optional[dict]=None):
    kwargs = {
        'secrets_file_path':settings.SECRETS_FILE_PATH, 'base_folder':'staff_portal',
        'secret_map':(settings.DB_USER_ALIAS, 'backend_apps.databases.%s' % settings.DB_USER_ALIAS),
        'driver_label':settings.DRIVER_LABEL, 'db_name':settings.DB_NAME,
        # TODO, for development and production environment, use configurable parameter
        # to optionally set multi_statement for the API endpoints that require to run
        # multiple SQL statements in one go.
    }
    if conn_args:
        kwargs['conn_args'] = conn_args
    return sqlalchemy_init_engine(**kwargs)


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


def _store_supervisor_validity(session, store_id:PositiveInt, usr_auth:dict):
    query = session.query(StoreProfile).filter(StoreProfile.id == store_id)
    saved_obj = query.first()
    if not saved_obj:
        raise FastApiHTTPException( detail={'code':'not_exist'},  headers={},
                status_code=FastApiHTTPstatus.HTTP_404_NOT_FOUND )
    if usr_auth['priv_status'] != ROLE_ID_SUPERUSER and saved_obj.supervisor_id != usr_auth['profile']:
        raise FastApiHTTPException( detail='Not allowed to edit the store profile',  headers={},
                status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
    return saved_obj


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
def delete_profile(request:DeleteStoreProfilesReqBody, user:dict=FastapiDepends(delete_profile_authorization)):
    session = request.metadata['session']
    deleting_objs = request.metadata['deleting_objs']
    for obj in deleting_objs:
        session.delete(obj)
    session.commit()
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
def edit_products_available(store_id:PositiveInt, request: AvailProductsReqBody, \
        user:dict=FastapiDepends(edit_products_authorization)):
    request.validate_products(staff_id=user['profile'])
    db_engine = _init_db_engine()
    try:
        with Session(bind=db_engine) as session:
            query = session.query(StoreProfile).filter(StoreProfile.id == store_id)
            saved_obj = query.first()
            if not saved_obj:
                raise FastApiHTTPException( detail={'code':'not_exist'},  headers={},
                        status_code=FastApiHTTPstatus.HTTP_404_NOT_FOUND )
            valid_staff_ids = list(map(lambda o:o.staff_id, saved_obj.staff))
            valid_staff_ids.append(saved_obj.supervisor_id)
            if user['profile'] not in valid_staff_ids:
                raise FastApiHTTPException( detail='Not allowed to edit the store products',  headers={},
                        status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN )
            new_prod_objs = list(map(lambda d: StoreProductAvailable(**d.dict()), request.__root__ ))
            saved_obj.products.clear()
            saved_obj.products.extend(new_prod_objs)
            session.commit()
    finally:
        db_engine.dispose()


