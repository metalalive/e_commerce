import logging

from django.conf    import settings as django_settings
from rest_framework.fields      import empty
from rest_framework.serializers import ModelSerializer
from rest_framework.exceptions  import ValidationError as RestValidationError

from common.util.python.async_tasks  import sendmail as async_send_mail

from ..models.base import _atomicity_fn, GenericUserProfile
from ..models.auth import UnauthResetAccountRequest

_logger = logging.getLogger(__name__)

class MailSourceValidator:
    err_msg_pattern = 'current email (ID=%s) comes from %s (ID=%s) , which is NOT a user profile'

    def __call__(self, value):
        model_cls = value.user_type.model_class()
        if model_cls is not GenericUserProfile:
            err_msg = self.err_msg_pattern % (value.id, model_cls.__name__, value.user_id)
            raise RestValidationError(err_msg)


class UnauthRstAccountReqSerializer(ModelSerializer):
    atomicity = _atomicity_fn
    class Meta:
        model = UnauthResetAccountRequest
        fields = ['email', 'time_created']
        read_only_fields = ['time_created',]

    def __init__(self, instance=None, data=empty, msg_template_path=None, subject_template=None,
            url_host=None, url_resource=None, account=None, **kwargs):
        # Following variables will be used for mailing with user authentication link
        # the user auth link could be for (1) account activation (2) username reset
        # (3) password reset
        self._msg_template_path = msg_template_path
        self._subject_template  = subject_template
        self._url_host     = url_host
        self._url_resource = url_resource
        self._account = account
        self.fields['email'].validators.append(MailSourceValidator())
        super().__init__(instance=None, data=data, **kwargs)

    def to_representation(self, instance):
        data = super().to_representation(instance=instance)
        if hasattr(self, '_async_tasks_id'):
            # will be present ONLY after saving validated data and issuing asynchronous mailing task
            task_id = self._async_tasks_id[instance.email.id]
            data['async_task'] = task_id
        return data

    def update(self, instance, validated_data):
        err_msg = 'update on reset account request is NOT allowed'
        raise RestValidationError(err_msg, code='not_supported')

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
        subject_data = {'first_name': profile.first_name}
        msg_data = {'first_name': profile.first_name, 'last_name': profile.last_name,
            'url_host': self._url_host, 'url_resource': self._url_resource, 'token':req.token,
            'expire_before': str(int(self.Meta.model.MAX_TOKEN_VALID_TIME / 60)),
        }
        to_addr = mail_ref.addr
        from_addr = django_settings.DEFAULT_FROM_EMAIL
        result = async_send_mail.delay(to_addrs=[to_addr], from_addr=from_addr,
                    subject_template=self._subject_template, subject_data=subject_data,
                    msg_template_path=self._msg_template_path,  msg_data=msg_data, )
        if not hasattr(self, '_async_tasks_id'):
            self._async_tasks_id = {}
        self._async_tasks_id[mail_ref.id] = result.task_id
        log_msg = ['_async_tasks_id', self._async_tasks_id]
        _logger.debug(None, *log_msg)

#### end of UnauthRstAccountReqSerializer


