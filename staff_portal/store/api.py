import logging
from functools import partial
from typing import List

from fastapi import APIRouter, Header, Depends as FastapiDepends
from fastapi import HTTPException as FastApiHTTPException, status as FastApiHTTPstatus
from fastapi.security  import OAuth2AuthorizationCodeBearer
from pydantic import PositiveInt
from sqlalchemy import delete as SqlAlDelete, select as SqlAlSelect, or_ as SqlAlOr, and_ as SqlAlAnd
from sqlalchemy.orm import Session

from common.auth.fastapi import base_authentication, base_permission_check
from common.models.constants  import ROLE_ID_SUPERUSER
from common.models.enums.base import AppCodeOptions

from .models import StoreProfile, StoreEmail, StorePhone, OutletLocation, StoreStaff, HourOfOperation, SaleableTypeEnum, StoreProductAvailable
from .shared import shared_ctx
from .validation import NewStoreProfilesReqBody, StoreProfileResponseBody, ExistingStoreProfileReqBody, \
        StoreSupervisorReqBody, StoreStaffsReqBody, BusinessHoursDaysReqBody, EditProductsReqBody, \
        StoreProfileReadResponseBody

app_code = AppCodeOptions.store.value[0]

_logger = logging.getLogger(__name__)

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
        tokenUrl=shared_ctx['settings'].REFRESH_ACCESS_TOKEN_API_URL,
        authorizationUrl="no_auth_url", )

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
    sa_new_stores = request.metadata['sa_new_stores']
    with shared_ctx['db_engine'].connect() as conn:
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
    with Session(bind=shared_ctx['db_engine']) as session:
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
    with Session(bind=shared_ctx['db_engine']) as _session:
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
def edit_staff(store_id:PositiveInt, request:StoreStaffsReqBody, \
        user:dict=FastapiDepends(edit_profile_authorization)):
    request.validate_staff(supervisor_id=user['profile'])
    with Session(bind=shared_ctx['db_engine']) as session:
        saved_obj = _store_supervisor_validity(session, store_id, usr_auth=user)
        staff_ids = list(map(lambda d: d.staff_id, request.__root__))
        stmt = SqlAlSelect(StoreStaff).where(StoreStaff.store_id == saved_obj.id) \
                .where(StoreStaff.staff_id.in_(staff_ids))
        result = session.execute(stmt)
        def _do_update(raw):
            saved_staff = raw[0]
            newdata = filter(lambda d: d.staff_id == saved_staff.staff_id, request.__root__)
            newdata = next(newdata)
            assert newdata is not None
            saved_staff.staff_id  = newdata.staff_id
            saved_staff.start_after = newdata.start_after
            saved_staff.end_before  = newdata.end_before
            return saved_staff.staff_id
        updatelist = tuple(map(_do_update, result))
        newdata = filter(lambda d: d.staff_id not in updatelist, request.__root__)
        new_staffs = map(lambda d:StoreStaff(**d.dict()), newdata)
        saved_obj.staff.extend(new_staffs)
        try:
            session.commit()
        except Exception as e:
            log_args = ['action', 'db-commit-error', 'detail', ','.join(e.args)]
            _logger.error(None, *log_args)
            raise FastApiHTTPException( detail={},  headers={},
                status_code=FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR )
    return None


@router.patch('/profile/{store_id}/business_hours',)
def edit_hours_operation(store_id:PositiveInt, request:BusinessHoursDaysReqBody, \
        user:dict=FastapiDepends(edit_profile_authorization)):
    with Session(bind=shared_ctx['db_engine']) as session:
        saved_obj = _store_supervisor_validity(session, store_id, usr_auth=user)
        new_time = list(map(lambda d:HourOfOperation(**d.dict()), request.__root__))
        saved_obj.open_days.clear()
        saved_obj.open_days.extend(new_time)
        session.commit()


@router.patch('/profile/{store_id}/products',)
def edit_products_available(store_id:PositiveInt, request: EditProductsReqBody, \
        user:dict=FastapiDepends(edit_products_authorization)):
    request.validate_products(staff_id=user['profile'])
    with Session(bind=shared_ctx['db_engine']) as session:
        saved_obj = _store_staff_validity(session, store_id, usr_auth=user)
        product_id_cond = map(lambda d: SqlAlAnd(
            StoreProductAvailable.product_type == d.product_type ,
            StoreProductAvailable.product_id == d.product_id )
            , request.__root__)
        find_product_condition = SqlAlOr(*product_id_cond)
        ## Don't use `saved_obj.products` generated by SQLAlchemy legacy Query API
        ## , instead I use `select` function to query relation fields
        stmt = SqlAlSelect(StoreProductAvailable) \
                .where(StoreProductAvailable.store_id == saved_obj.id) \
                .where(find_product_condition)
        result = session.execute(stmt)
        def _do_update(saved_product): # tuple
            saved_product = saved_product[0]
            newdata = filter(lambda d: d.product_type is saved_product.product_type
                    and d.product_id == saved_product.product_id, request.__root__)
            newdata = next(newdata)
            assert newdata is not None
            saved_product.price = newdata.price
            saved_product.start_after = newdata.start_after
            saved_product.end_before = newdata.end_before
            return (newdata.product_type, newdata.product_id)
        updatelist = tuple(map(_do_update, result))
        newdata = filter(lambda d: (d.product_type, d.product_id) not in updatelist, request.__root__)
        new_model_fn = lambda d: StoreProductAvailable(store_id=saved_obj.id, **d.dict())
        new_products = map(new_model_fn, newdata)
        session.add_all([*new_products])
        try:
            session.commit()
        except Exception as e:
            log_args = ['action', 'db-commit-error', 'detail', ','.join(e.args)]
            _logger.error(None, *log_args)
            raise FastApiHTTPException( detail={},  headers={},
                status_code=FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR )


@router.delete('/profile/{store_id}/products', status_code=FastApiHTTPstatus.HTTP_204_NO_CONTENT)
def discard_store_products(store_id:PositiveInt, pitems:str, ppkgs:str, \
        user:dict=FastapiDepends(edit_products_authorization)):
    try:
        pitems = list(map(int, pitems.split(',')))
        ppkgs  = list(map(int, ppkgs.split(',')))
        if len(pitems) == 0 and len(ppkgs) == 0:
            raise FastApiHTTPException( detail={'ids':'empty'},  headers={},
                    status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
    except ValueError as e:
        raise FastApiHTTPException( detail={'ids':'invalid-id', 'detail':e.args},
                headers={}, status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST )
    with Session(bind=shared_ctx['db_engine']) as _session:
        saved_store = _store_staff_validity(_session, store_id, usr_auth=user)
        _cond_fn = lambda d, t: SqlAlAnd(StoreProductAvailable.product_type == t,
            StoreProductAvailable.product_id == d)
        pitem_cond = map(partial(_cond_fn, t=SaleableTypeEnum.ITEM), pitems)
        ppkg_cond  = map(partial(_cond_fn, t=SaleableTypeEnum.PACKAGE), ppkgs)
        find_product_condition = SqlAlOr(*pitem_cond, *ppkg_cond)
        stmt = SqlAlDelete(StoreProductAvailable) \
                .where(StoreProductAvailable.store_id == saved_store.id) \
                .where(find_product_condition)
        result = _session.execute(stmt)
        # print generated raw SOL with actual values
        # str(stmt.compile(compile_kwargs={"literal_binds": True}))
        _session.commit()
    if result.rowcount == 0:
        raise FastApiHTTPException( detail={},  headers={},
                status_code=FastApiHTTPstatus.HTTP_410_GONE )
    return None


@router.get('/profile/{store_id}/products', response_model=EditProductsReqBody)
def read_profile_products(store_id:PositiveInt, user:dict=FastapiDepends(common_authentication)):
    # TODO, figure out how to handle large dataset, pagination or other techniques
    with Session(bind=shared_ctx['db_engine']) as session:
        saved_obj = _store_staff_validity(session, store_id, usr_auth=user)
        response = EditProductsReqBody.from_orm(saved_obj.products)
    return response


@router.get('/profile/{store_id}', response_model=StoreProfileReadResponseBody)
def read_profile(store_id:PositiveInt, user:dict=FastapiDepends(common_authentication)):
    with Session(bind=shared_ctx['db_engine']) as session:
        saved_obj = _store_staff_validity(session, store_id, usr_auth=user)
        response = StoreProfileReadResponseBody.from_orm(saved_obj)
    return response

