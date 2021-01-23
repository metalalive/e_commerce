import logging

from django.contrib.contenttypes.models  import ContentType
from django.utils.deconstruct   import deconstructible
from django.core.validators     import MaxValueValidator
from rest_framework.fields      import empty

_logger = logging.getLogger(__name__)
# logstash will duplicate the same log message, figure out how that happenes.

class QuotaCheckerMixin:
    @deconstructible
    class QuotaValidator(MaxValueValidator):
        def __call__(self, value):
            value = len(value)
            log_msg = ['value', value, 'limit_value', self.limit_value]
            _logger.debug(None, *log_msg)
            super().__call__(value)

    def __init__(self, instance=None, data=empty, **kwargs):
        super().__init__(instance=instance, data=data, **kwargs)
        errmsg = 'you haven\'t configured quota for the item'
        self._quota_validator = self.QuotaValidator(limit_value=0, message=errmsg)
        self.validators.append(self._quota_validator)

    @property
    def applied_quota(self):
        if not hasattr(self, '_applied_quota'):
            self._applied_quota = 0
        return self._applied_quota

    @applied_quota.setter
    def applied_quota(self, value):
        if value :
            ct_model_cls = ContentType.objects.get(model=self.child.Meta.model.__name__)
            self._applied_quota = value.get(ct_model_cls, 0)
        else:
            self._applied_quota = 0
        errmsg = 'number of items provided exceeds the limit: {q0}'
        errmsg = errmsg.format(q0=str(self._applied_quota))
        self._quota_validator.message = errmsg
        self._quota_validator.limit_value = self._applied_quota
        log_msg = ['srlz_cls', type(self.child).__qualname__, '_applied_quota', self._applied_quota]
        _logger.debug(None, *log_msg)


