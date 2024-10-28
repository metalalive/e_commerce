import re
import logging

from django.conf import settings as django_settings
from django.core.exceptions import (
    ObjectDoesNotExist,
    ValidationError as DjangoValidationError,
)
from django.core.validators import BaseValidator as DjangoBaseValidator
from django.utils import timezone as django_timezone
from django.utils.deconstruct import deconstructible
from django.contrib.auth import password_validation, get_user_model
from django.contrib.auth.hashers import check_password

from rest_framework.fields import empty
from rest_framework.serializers import ModelSerializer, Serializer
from rest_framework.fields import CharField
from rest_framework.exceptions import (
    ValidationError as RestValidationError,
    ErrorDetail as DRFErrorDetail,
)

from ecommerce_common.util.async_tasks import sendmail as async_send_mail

from ..util import render_mail_content
from ..models.base import _atomicity_fn, GenericUserProfile
from ..models.auth import UnauthResetAccountRequest

_logger = logging.getLogger(__name__)


class MailSourceValidator:
    err_msg_pattern = (
        "current email (ID=%s) comes from %s (ID=%s) , which is NOT a user profile"
    )

    def __call__(self, value):
        model_cls = value.user_type.model_class()
        if model_cls is not GenericUserProfile:
            err_msg = self.err_msg_pattern % (
                value.id,
                model_cls.__name__,
                value.user_id,
            )
            raise RestValidationError(err_msg)


class UnauthRstAccountReqSerializer(ModelSerializer):
    atomicity = _atomicity_fn

    class Meta:
        model = UnauthResetAccountRequest
        fields = ["email", "time_created"]
        read_only_fields = [
            "time_created",
        ]

    def __init__(
        self,
        instance=None,
        data=empty,
        msg_template_path=None,
        subject_template=None,
        url_host=None,
        url_resource=None,
        account=None,
        **kwargs
    ):
        # Following variables will be used for mailing with user authentication link
        # the user auth link could be for (1) account activation (2) username reset
        # (3) password reset
        self._msg_template_path = msg_template_path
        self._subject_template = subject_template
        self._url_host = url_host
        self._url_resource = url_resource
        self._account = account
        self.fields["email"].validators.append(MailSourceValidator())
        super().__init__(instance=None, data=data, **kwargs)

    def to_representation(self, instance):
        data = super().to_representation(instance=instance)
        if hasattr(self, "_async_tasks_id"):
            # will be present ONLY after saving validated data and issuing asynchronous mailing task
            task_id = self._async_tasks_id[instance.email.id]
            data["async_task"] = task_id
        return data

    def update(self, instance, validated_data):
        err_msg = "update on reset account request is NOT allowed"
        raise RestValidationError(err_msg, code="not_supported")

    def create(self, validated_data):
        instance = super().create(validated_data=validated_data)
        self._mailing(req=instance)
        return instance

    def _mailing(self, req):
        """
        get mail plaintext (instead of passing model instance as task argument),
        get mail template, and data to render, place all of them to task queue
        , the mailing process will be done asynchronously by another service program
        """
        mail_ref = req.email
        prof_model_cls = mail_ref.user_type.model_class()
        profile = prof_model_cls.objects.get(id=mail_ref.user_id)
        subject_data = {"first_name": profile.first_name}
        msg_data = {
            "first_name": profile.first_name,
            "last_name": profile.last_name,
            "url_host": self._url_host,
            "url_resource": self._url_resource,
            "token": req.token,
            "expire_before": str(int(self.Meta.model.MAX_TOKEN_VALID_TIME / 60)),
        }
        to_addr = mail_ref.addr
        from_addr = django_settings.DEFAULT_FROM_EMAIL
        content, subject = render_mail_content(
            msg_template_path=self._msg_template_path,
            msg_data=msg_data,
            subject_template_path=self._subject_template,
            subject_data=subject_data,
        )
        result = async_send_mail.delay(
            to_addrs=[to_addr], from_addr=from_addr, subject=subject, content=content
        )
        if not hasattr(self, "_async_tasks_id"):
            self._async_tasks_id = {}
        self._async_tasks_id[mail_ref.id] = result.task_id
        log_msg = ["_async_tasks_id", self._async_tasks_id]
        _logger.debug(None, *log_msg)


#### end of UnauthRstAccountReqSerializer


@deconstructible
class UsernameUniquenessValidator:
    """
    give model class and name of a field, check record uniqueness
    associated with giving value in __call__ function
    """

    def __init__(self, account, **kwargs):
        self._account = account or get_user_model()()

    def __call__(self, value):
        account = self._account
        errmsg = None
        log_level = logging.INFO
        if account.pk and account.username == value:
            errmsg = "your new username should be different from original one"
        else:
            backup = account.username
            try:  # invoke existing validator at model level
                account.username = value
                account.validate_unique()
            except DjangoValidationError as e:
                err_list = e.error_dict.get(self._account.USERNAME_FIELD, [])
                err_list = tuple(
                    filter(
                        lambda item: item.message.find("already exist") > 0, err_list
                    )
                )
                if any(err_list):
                    errmsg = err_list[0].message
                    log_level = logging.WARNING
            finally:
                account.username = backup
        if errmsg:
            log_msg = [
                "errmsg",
                errmsg,
                "value",
                value,
                "account_id",
                account.pk,
                "account_username",
                account.username,
            ]
            _logger.log(log_level, None, *log_msg)
            raise RestValidationError(errmsg)


@deconstructible
class PasswordComplexityValidator:
    def __init__(self, account, password_confirm=None):
        self._account = account or get_user_model()()
        if not password_confirm is None:
            self._password_confirm = password_confirm

    def __call__(self, value):
        errs = []
        if hasattr(self, "_password_confirm"):
            if self._password_confirm != value:
                msg = "'password' field doesn't match 'confirm password' field."
                errs.append(DRFErrorDetail(msg, code="confirm_fail"))
        if re.search("[^\\w]", value) is None:
            msg = "new password must contain at least one special symbol e.g. @, $, +, ...."
            errs.append(DRFErrorDetail(msg, code="special_char_required"))
        try:
            password_validation.validate_password(value, self._account)
        except RestValidationError as e:
            errs.extend(e.error_list)
        if any(errs):
            log_msg = ["errs", errs]
            _logger.info(None, *log_msg)
            raise RestValidationError(errs)


@deconstructible
class StringEqualValidator(DjangoBaseValidator):
    code = "invalid"

    def compare(self, a, b):
        return a != b


@deconstructible
class OldPasswdValidator(DjangoBaseValidator):
    code = "invalid"

    def compare(self, a, b):
        # check password without saving it
        return not check_password(password=a, encoded=b)


class LoginAccountSerializer(Serializer):
    """
    There are case scenarios that will invoke this serializer :
        case #1: New users activate their own login account at the first time
        case #2: Unauthorized users forget their username, and request to reset
        case #3: Unauthorized users forget their password, and request to reset
        case #4: Authorized users change their username, within valid login session
        case #5: Authorized users change their password, within valid login session
        case #6: Login authentication
    """

    # case #1: rst_req = non-null, account = null
    # case #2: rst_req = non-null, account = null, but can be derived from rst_req
    # case #3: rst_req = non-null, account = null, but can be derived from rst_req
    # case #4: rst_req = null, account = non-null
    # case #5: rst_req = null, account = non-null
    def __init__(
        self,
        data,
        account,
        rst_req,
        confirm_passwd=False,
        uname_required=False,
        old_uname_required=False,
        old_passwd_required=False,
        passwd_required=False,
        mail_kwargs=None,
        **kwargs
    ):
        log_msg = [
            "account",
            account,
        ]
        self._rst_req = rst_req
        self._mail_kwargs = mail_kwargs
        if account and account.is_authenticated:
            self._profile = account.profile
        elif rst_req:
            prof_model_cls = rst_req.email.user_type.model_class()
            self._profile = prof_model_cls.objects.get(id=rst_req.email.user_id)
            try:
                account = self._profile.account
            except ObjectDoesNotExist as e:
                account = None
        else:
            errmsg = "caller must provide either `account` or `rst_req`, both of them must NOT be null"
            log_msg.extend(["errmsg", errmsg])
            _logger.error(None, *log_msg)
            raise AssertionError(errmsg)

        if uname_required:
            uname_validator = UsernameUniquenessValidator(account=account)
            self.fields["username"] = CharField(
                required=True,
                max_length=32,
                min_length=8,
            )
            self.fields["username"].validators.append(uname_validator)
        if passwd_required:
            passwd2 = data.get("password2", "") if confirm_passwd else None
            passwd_validator = PasswordComplexityValidator(
                account=account, password_confirm=passwd2
            )
            self.fields["password"] = CharField(
                required=True,
                max_length=128,
                min_length=12,
            )
            self.fields["password"].validators.append(passwd_validator)
        if confirm_passwd:
            self.fields["password2"] = CharField(required=True, max_length=128)
        if old_uname_required:
            old_uname_validator = StringEqualValidator(
                limit_value=account.username, message="incorrect old username"
            )
            self.fields["old_uname"] = CharField(required=True, max_length=32)
            self.fields["old_uname"].validators.append(old_uname_validator)
        if old_passwd_required:
            old_passwd_validator = OldPasswdValidator(
                limit_value=account.password, message="incorrect old password"
            )
            self.fields["old_passwd"] = CharField(required=True, max_length=128)
            self.fields["old_passwd"].validators.append(old_passwd_validator)

        _logger.debug(None, *log_msg)
        super().__init__(instance=account, data=data, **kwargs)

    def _clean_validate_only_fields(self, validated_data):
        for key in ["password2", "old_uname", "old_passwd"]:
            validated_data.pop(key, None)
        log_msg = ["validated_data", validated_data]
        _logger.debug(None, *log_msg)
        return validated_data

    def create(self, validated_data):
        profile = self._profile
        email = self._rst_req.email
        validated_data = self._clean_validate_only_fields(validated_data)
        validated_data["password_last_updated"] = django_timezone.now()
        with _atomicity_fn():
            instance = profile.activate(new_account_data=validated_data)
            self._rst_req.delete()
        if self._mail_kwargs and email:  # notify user again by email
            self._mailing(profile=profile, mail_ref=email, username=instance.username)
        return instance

    def update(self, instance, validated_data):
        profile = self._profile
        email = None
        validated_data = self._clean_validate_only_fields(validated_data)
        with _atomicity_fn():
            for attr, value in validated_data.items():
                if attr == "password":
                    instance.set_password(raw_password=value)
                    instance.password_last_updated = django_timezone.now()
                else:
                    setattr(instance, attr, value)
            instance.save()  # password will be hashed in AuthUser model before save
            if self._rst_req:
                email = self._rst_req.email
                self._rst_req.delete()
            # check instance.username and instance.password if necessary
        if self._mail_kwargs and email:
            self._mailing(profile=profile, mail_ref=email, username=instance.username)
        return instance

    def _mailing(self, profile, mail_ref, username):
        event_time = django_timezone.now()
        masked_username = username[:3]
        msg_data = {
            "first_name": profile.first_name,
            "last_name": profile.last_name,
            "event_time": event_time,
            "masked_username": masked_username,
        }
        to_addr = mail_ref.addr
        from_addr = django_settings.DEFAULT_FROM_EMAIL

        content, subject = render_mail_content(
            msg_template_path=self._mail_kwargs["msg_template_path"],
            msg_data=msg_data,
            subject_template_path=self._mail_kwargs["subject_template"],
        )
        result = async_send_mail.delay(
            to_addrs=[to_addr],
            from_addr=from_addr,
            subject=subject,
            content=content,
        )
        if not hasattr(self, "_async_tasks_id"):
            self._async_tasks_id = {}
        self._async_tasks_id[profile.pk] = result.task_id


#### end of LoginAccountSerializer
