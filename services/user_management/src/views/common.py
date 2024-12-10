import logging

from django.core.exceptions import (
    ObjectDoesNotExist,
    MultipleObjectsReturned,
    PermissionDenied,
)
from rest_framework.response import Response as RestResponse

from ..models.base import EmailAddress, GenericUserProfile
from ..models.auth import UnauthResetAccountRequest

_logger = logging.getLogger(__name__)


def check_auth_req_token(fn_succeed, fn_failure):
    def inner(self, request, *args, **kwargs):
        token = kwargs.get("token", None)
        rst_req = UnauthResetAccountRequest.get_request(token_urlencoded=token)
        if rst_req:
            kwargs["rst_req"] = rst_req
            resp_kwargs = fn_succeed(self, request, *args, **kwargs)
        else:
            resp_kwargs = fn_failure(self, request, *args, **kwargs)
        return RestResponse(**resp_kwargs)

    return inner


## --------------------------------------------------------------------------------------
# process single user at a time
# create new request in UnauthResetAccountRequest for either changing username or password
# Send mail with account-reset URL page to the user


def get_profile_by_email(addr: str, request):
    try:
        email = EmailAddress.objects.get(addr=addr)
        prof_cls = email.user_type.model_class()
        if prof_cls is not GenericUserProfile:
            raise MultipleObjectsReturned(
                "invalid class type for individual user profile"
            )
        profile = prof_cls.objects.get(pk=email.user_id)
        if not profile.account.is_active:
            raise PermissionDenied("not allowed to query account of a deactivated user")
    except (ObjectDoesNotExist, MultipleObjectsReturned, PermissionDenied) as e:
        fully_qualified_cls_name = "%s.%s" % (type(e).__module__, type(e).__qualname__)
        err_args = [
            "email",
            addr,
            "excpt_type",
            fully_qualified_cls_name,
            "excpt_msg",
            e.args[0],
        ]
        _logger.warning(None, *err_args, request=request)
        email = None
        profile = None
    return email, profile
