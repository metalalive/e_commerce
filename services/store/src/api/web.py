import logging
from typing import Optional, List

from jwt.exceptions import (
    DecodeError,
    ExpiredSignatureError,
    ImmatureSignatureError,
    InvalidAudienceError,
    InvalidIssuedAtError,
    InvalidIssuerError,
    MissingRequiredClaimError,
    InvalidKeyError,
    PyJWKClientConnectionError,
)
from fastapi import APIRouter, Depends as FastapiDepends
from fastapi import HTTPException as FastApiHTTPException, status as FastApiHTTPstatus
from fastapi.security import OAuth2AuthorizationCodeBearer
from pydantic import PositiveInt
from sqlalchemy import delete as SqlAlDelete
from sqlalchemy.ext.asyncio import AsyncSession

from ecommerce_common.auth.auth import base_authentication, base_permission_check
from ecommerce_common.models.constants import ROLE_ID_SUPERUSER
from ecommerce_common.models.enums.base import AppCodeOptions

from ..models import (
    StoreProfile,
    StoreEmail,
    StorePhone,
    StoreStaff,
    HourOfOperation,
    SaleableTypeEnum,
    StoreProductAvailable,
)
from ..shared import shared_ctx
from ..validation import (
    NewStoreProfilesReqBody,
    ExistingStoreProfileReqBody,
    StoreSupervisorReqBody,
    StoreStaffsReqBody,
    BusinessHoursDaysReqBody,
    EditProductsReqBody,
)
from ..dto import StoreProfileDto, StoreProfileCreatedDto

app_code = AppCodeOptions.store.value[0]

_logger = logging.getLogger(__name__)

router = APIRouter(
    prefix="",  # could be API versioning e.g. /v0.0.1/* ,  /v2.0.1/*
    tags=["generic_store"],
    # TODO: dependencies are function executed before hitting the API endpoint
    # , (like router-level middleware for all downstream endpoints ?)
    dependencies=[],
    # the argument `lifespan` hasn't been integrated well in FastAPI an Starlette
    responses={
        FastApiHTTPstatus.HTTP_404_NOT_FOUND: {"description": "resource not found"},
        FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR: {
            "description": "internal server error"
        },
    },
)


oauth2_scheme = OAuth2AuthorizationCodeBearer(
    tokenUrl=shared_ctx["settings"].REFRESH_ACCESS_TOKEN_API_URL,
    authorizationUrl="no_auth_url",
)


async def common_authentication(encoded_token: str = FastapiDepends(oauth2_scheme)):
    try:
        return base_authentication(
            token=encoded_token,
            audience=["store"],
            keystore=shared_ctx["auth_keystore"],
        )
    except PyJWKClientConnectionError as e:
        log_args = ["action", "jwt-auth-error", "detail", str(e)]
        _logger.warning(None, *log_args)
        raise FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="internal-error",
            headers={"www-Authenticate": "Bearer"},
        )
    except (
        TypeError,
        DecodeError,
        ExpiredSignatureError,
        ImmatureSignatureError,
        InvalidAudienceError,
        InvalidIssuedAtError,
        InvalidIssuerError,
        MissingRequiredClaimError,
        InvalidKeyError,
    ) as e:
        log_args = ["action", "jwt-auth-error", "detail", str(e)]
        _logger.warning(None, *log_args)
        raise FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_401_UNAUTHORIZED,
            detail="authentication-failure",
            headers={"www-Authenticate": "Bearer"},
        )


class Authorization:
    def __init__(self, app_code, perm_codes):
        self._app_code = app_code
        self._perm_codes = set(perm_codes)

    def __call__(self, user: dict = FastapiDepends(common_authentication)):
        # low-level permissions check
        error_obj = FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
            detail="Permission check failure",
            headers={},
        )
        user = base_permission_check(
            user=user,
            app_code=self._app_code,
            required_perm_codes=self._perm_codes,
            error_obj=error_obj,
        )
        return user


add_profile_authorization = Authorization(
    app_code=app_code, perm_codes=["view_storeprofile", "add_storeprofile"]
)
edit_profile_authorization = Authorization(
    app_code=app_code, perm_codes=["view_storeprofile", "change_storeprofile"]
)
switch_supervisor_authorization = Authorization(
    app_code=app_code, perm_codes=["view_storeprofile", "change_storeprofile"]
)
delete_profile_authorization = Authorization(
    app_code=app_code, perm_codes=["view_storeprofile", "delete_storeprofile"]
)
edit_products_authorization = Authorization(
    app_code=app_code,
    perm_codes=[
        "add_storeproductavailable",
        "change_storeproductavailable",
        "delete_storeproductavailable",
    ],
)


async def _storefront_existence_validity(
    session: AsyncSession,
    store_id: PositiveInt,
    eager_load_columns: Optional[List] = None,
) -> StoreProfile:
    saved_obj = await StoreProfile.try_load(session, store_id, eager_load_columns)
    if not saved_obj:
        raise FastApiHTTPException(
            detail={"code": "not_exist"},
            headers={},
            status_code=FastApiHTTPstatus.HTTP_404_NOT_FOUND,
        )
    return saved_obj


async def _storefront_supervisor_validity(
    session: AsyncSession,
    store_id: PositiveInt,
    usr_auth: dict,
    eager_load_columns: Optional[List] = None,
) -> StoreProfile:
    saved_obj = await _storefront_existence_validity(
        session, store_id, eager_load_columns
    )  # `supervisor_id` does not need to be added to `eager_load_columns`
    if (
        usr_auth["priv_status"] != ROLE_ID_SUPERUSER
        and saved_obj.supervisor_id != usr_auth["profile"]
    ):
        raise FastApiHTTPException(
            detail="Not allowed to edit the store profile",
            headers={},
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
        )
    return saved_obj


async def _storefront_staff_validity(
    session: AsyncSession,
    store_id: PositiveInt,
    usr_auth: dict,
    eager_load_columns: Optional[List] = None,
) -> StoreProfile:
    if eager_load_columns:
        eager_load_columns.append(StoreProfile.staff)
    else:
        eager_load_columns = [StoreProfile.staff]
    saved_obj = await _storefront_existence_validity(
        session, store_id, eager_load_columns
    )
    valid_staff_ids = list(map(lambda o: o.staff_id, saved_obj.staff))
    valid_staff_ids.append(saved_obj.supervisor_id)
    if usr_auth["profile"] not in valid_staff_ids:
        raise FastApiHTTPException(
            detail="Not allowed to edit the store products",
            headers={},
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
        )
    return saved_obj


@router.post(
    "/profiles",
    status_code=FastApiHTTPstatus.HTTP_201_CREATED,
    response_model=List[StoreProfileCreatedDto],
)
async def add_profiles(
    request: NewStoreProfilesReqBody,
    user: dict = FastapiDepends(add_profile_authorization),
):
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        sa_new_stores = await request.validate_quota(session)
        await StoreProfile.bulk_insert(objs=sa_new_stores, session=session)
        for obj in sa_new_stores:
            await session.refresh(obj, attribute_names=["id", "supervisor_id"])

        def _fn(obj):
            return StoreProfileCreatedDto(id=obj.id, supervisor_id=obj.supervisor_id)

        resp_data = list(map(_fn, sa_new_stores))
    return resp_data


## def add_profiles()


@router.patch("/profile/{store_id}")
async def edit_profile(
    store_id: PositiveInt,
    request: ExistingStoreProfileReqBody,
    user: dict = FastapiDepends(edit_profile_authorization),
):
    # part of authorization has to be handled at here because it requires all these arguments
    quota_arrangement = dict(map(lambda d: (d["mat_code"], d["maxnum"]), user["quota"]))
    num_new_items = len(request.emails)
    max_limit = quota_arrangement.get(StoreEmail.quota_material.value, 0)
    if max_limit < num_new_items:
        err_msg = "Limit exceeds, num_new_items:%s, max_limit:%s" % (
            num_new_items,
            max_limit,
        )
        raise FastApiHTTPException(
            detail={"emails": [err_msg]},
            headers={},
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
        )
    num_new_items = len(request.phones)
    max_limit = quota_arrangement.get(StorePhone.quota_material.value, 0)
    if max_limit < num_new_items:
        err_msg = "Limit exceeds, num_new_items:%s, max_limit:%s" % (
            num_new_items,
            max_limit,
        )
        raise FastApiHTTPException(
            detail={"phones": [err_msg]},
            headers={},
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
        )
    # TODO, figure out better way to authorize with database connection
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        related_attributes = [
            StoreProfile.emails,
            StoreProfile.phones,
            StoreProfile.location,
        ]
        saved_obj = await _storefront_supervisor_validity(
            session, store_id, usr_auth=user, eager_load_columns=related_attributes
        )
        saved_obj.update(request)
        await session.commit()
    return None


@router.patch("/profile/{store_id}/supervisor")
async def switch_supervisor(
    store_id: PositiveInt,
    request: StoreSupervisorReqBody,
    user: dict = FastapiDepends(switch_supervisor_authorization),
):
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        await request.validate_quota(session)
        saved_obj = await _storefront_supervisor_validity(
            session, store_id, usr_auth=user
        )
        saved_obj.supervisor_id = request.supervisor_id
        await session.commit()
    return None


@router.delete("/profiles", status_code=FastApiHTTPstatus.HTTP_204_NO_CONTENT)
async def delete_profile(
    ids: str, user: dict = FastapiDepends(delete_profile_authorization)
):
    try:
        ids = list(map(int, ids.split(",")))
        if len(ids) == 0:
            raise FastApiHTTPException(
                detail={"ids": "empty"},
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
    except ValueError:
        raise FastApiHTTPException(
            detail={"ids": "invalid-id"},
            headers={},
            status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
        )
    async with AsyncSession(bind=shared_ctx["db_engine"]) as _session:
        # TODO, staff validity check if any staff member of the shop exists
        stmt = SqlAlDelete(StoreProfile).where(StoreProfile.id.in_(ids))
        result = await _session.execute(stmt)  # TODO, consider soft-delete
        for s_id in ids:
            emit_event_edit_products(
                s_id, rpc_hdlr=shared_ctx["order_app_rpc"], remove_all=True
            )
        await _session.commit()
    # Note python does not have `scope` concept, I can access the variables
    # `result` and `ids` declared above.
    if result.rowcount == 0:
        raise FastApiHTTPException(
            detail={}, headers={}, status_code=FastApiHTTPstatus.HTTP_410_GONE
        )
    return None


@router.patch("/profile/{store_id}/staff")
async def edit_staff(
    store_id: PositiveInt,
    request: StoreStaffsReqBody,
    user: dict = FastapiDepends(edit_profile_authorization),
):
    request.validate_staff(supervisor_id=user["profile"])
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        saved_obj = await _storefront_supervisor_validity(
            session, store_id, usr_auth=user, eager_load_columns=[StoreProfile.staff]
        )
        saved_staffs = await StoreStaff.try_load(
            session, store_id=saved_obj.id, reqdata=request.root
        )
        updatelist = StoreStaff.bulk_update(saved_staffs, request.root)

        newdata = filter(lambda d: d.staff_id not in updatelist, request.root)
        new_staffs = map(lambda d: StoreStaff(**d.model_dump()), newdata)
        saved_obj.staff.extend(new_staffs)
        try:
            await session.commit()
        except Exception as e:
            log_args = ["action", "db-commit-error", "detail", ",".join(e.args)]
            _logger.error(None, *log_args)
            raise FastApiHTTPException(
                detail={},
                headers={},
                status_code=FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR,
            )
    return None


@router.patch("/profile/{store_id}/business_hours")
async def edit_hours_operation(
    store_id: PositiveInt,
    request: BusinessHoursDaysReqBody,
    user: dict = FastapiDepends(edit_profile_authorization),
):
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        saved_obj = await _storefront_supervisor_validity(
            session,
            store_id,
            usr_auth=user,
            eager_load_columns=[StoreProfile.open_days],
        )
        new_time = list(map(lambda d: HourOfOperation(**d.model_dump()), request.root))
        saved_obj.open_days.clear()
        saved_obj.open_days.extend(new_time)
        await session.commit()


def emit_event_edit_products(
    _store_id: int,
    rpc_hdlr,
    remove_all: bool = False,
    s_currency: Optional[str] = None,
    updating: Optional[List[StoreProductAvailable]] = None,
    creating: Optional[List[StoreProductAvailable]] = None,
    deleting: Optional[dict] = None,
):
    # currently this service uses server-side timezone
    # TODO, switch to the time zones provided from client if required
    def convertor(obj):
        return {
            "price": obj.price,
            "start_after": obj.start_after.astimezone().isoformat(),
            "end_before": obj.end_before.astimezone().isoformat(),
            "product_type": obj.product_type.value,
            "product_id": obj.product_id,
        }

    _updating = map(convertor, updating) if updating else []
    _creating = map(convertor, creating) if creating else []
    _deleting = deleting or {}
    _deleting.update(
        {
            "item_type": SaleableTypeEnum.ITEM.value,
            "pkg_type": SaleableTypeEnum.PACKAGE.value,
        }
    )
    kwargs = {
        "s_id": _store_id,
        "currency": s_currency,
        "rm_all": remove_all,
        "deleting": _deleting,
        "updating": [*_updating],
        "creating": [*_creating],
    }
    remote_fn = rpc_hdlr.update_store_products
    remote_fn.enable_confirm = True
    reply_evt = remote_fn(**kwargs)
    # Note this application is NOT responsible to create the RPC
    # queue for order-processing application
    rpc_response = reply_evt.result
    # publish-confirm is enable here, this function only cares whether the message is sucessfully
    # sent to message broker in the middle, the broker is responsible to prevent message loss
    # currently I use RabbitMQ with durable queue so the data safety should be guaranteed.
    if rpc_response["status"] != reply_evt.status_opt.INITED:
        log_args = [
            "action",
            "rpc-publish",
            "status",
            rpc_response["status"],
            "detail",
            str(rpc_response["result"]),
            "extra_err",
            rpc_response.get("error", "N/A"),
        ]
        _logger.error(None, *log_args)
        raise FastApiHTTPException(
            detail={},
            headers={},
            status_code=FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR,
        )
    # TODO, better design option for data consistency between `storefront` and `order` app


@router.patch("/profile/{store_id}/products")
async def edit_products_available(
    store_id: PositiveInt,
    request: EditProductsReqBody,
    user: dict = FastapiDepends(edit_products_authorization),
):
    request.validate_products(staff_id=user["profile"])
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        saved_obj = await _storefront_staff_validity(session, store_id, usr_auth=user)
        updating_products = await StoreProductAvailable.try_load(
            session, store_id=saved_obj.id, reqdata=request.root
        )
        updatelist = StoreProductAvailable.bulk_update(updating_products, request.root)
        new_products = [
            StoreProductAvailable(store_id=saved_obj.id, **d.model_dump())
            for d in request.root
            if (d.product_type, d.product_id) not in updatelist
        ]
        session.add_all([*new_products])
        emit_event_edit_products(
            store_id,
            s_currency=saved_obj.currency.value,
            rpc_hdlr=shared_ctx["order_app_rpc"],
            updating=updating_products,
            creating=new_products,
        )
        try:
            await session.commit()
        except Exception as e:
            log_args = ["action", "db-commit-error", "detail", ",".join(e.args)]
            _logger.error(None, *log_args)
            raise FastApiHTTPException(
                detail={},
                headers={},
                status_code=FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR,
            )


@router.delete(
    "/profile/{store_id}/products", status_code=FastApiHTTPstatus.HTTP_204_NO_CONTENT
)
async def discard_store_products(
    store_id: PositiveInt,
    pitems: str,
    ppkgs: str,
    user: dict = FastapiDepends(edit_products_authorization),
):
    try:
        pitems = list(map(int, pitems.split(",")))
        ppkgs = list(map(int, ppkgs.split(",")))
        if len(pitems) == 0 and len(ppkgs) == 0:
            raise FastApiHTTPException(
                detail={"ids": "empty"},
                headers={},
                status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            )
    except ValueError as e:
        raise FastApiHTTPException(
            detail={"ids": "invalid-id", "detail": e.args},
            headers={},
            status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
        )

    async with AsyncSession(bind=shared_ctx["db_engine"]) as _session:
        saved_store = await _storefront_staff_validity(
            _session, store_id, usr_auth=user
        )
        num_deleted = await StoreProductAvailable.bulk_delete(
            _session, saved_store.id, pitems, ppkgs
        )
        emit_event_edit_products(
            store_id,
            s_currency=saved_store.currency.value,
            rpc_hdlr=shared_ctx["order_app_rpc"],
            deleting={"items": pitems, "pkgs": ppkgs},
        )
        # print generated raw SOL with actual values
        # str(stmt.compile(compile_kwargs={"literal_binds": True}))
        await _session.commit()
    if num_deleted == 0:
        raise FastApiHTTPException(
            detail={}, headers={}, status_code=FastApiHTTPstatus.HTTP_410_GONE
        )
    return None


## end of def discard_store_products


@router.get("/profile/{store_id}/products", response_model=EditProductsReqBody)
async def read_profile_products(
    store_id: PositiveInt, user: dict = FastapiDepends(common_authentication)
):
    # TODO, figure out how to handle large dataset, pagination or other techniques
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        saved_obj = await _storefront_staff_validity(
            session, store_id, usr_auth=user, eager_load_columns=[StoreProfile.products]
        )
        response = EditProductsReqBody.model_validate(saved_obj.products)
    return response


@router.get("/profile/{store_id}", response_model=StoreProfileDto)
async def read_profile(
    store_id: PositiveInt, user: dict = FastapiDepends(common_authentication)
):
    async with AsyncSession(bind=shared_ctx["db_engine"]) as session:
        related_attributes = [
            StoreProfile.phones,
            StoreProfile.emails,
            StoreProfile.location,
            StoreProfile.open_days,
            StoreProfile.staff,
        ]
        saved_obj = await _storefront_staff_validity(
            session, store_id, usr_auth=user, eager_load_columns=related_attributes
        )
        response = StoreProfileDto.model_validate(saved_obj)
    return response
