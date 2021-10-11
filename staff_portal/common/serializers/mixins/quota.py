import logging

_logger = logging.getLogger(__name__)

class BaseQuotaCheckerMixin:
    def __init__(self, quota_validator, **kwargs):
        super().__init__(**kwargs)
        self._quota_validator = quota_validator
        self._applied_quota = 0

    @property
    def applied_quota(self):
        return self._applied_quota

    @applied_quota.setter
    def applied_quota(self, value):
        log_msg = ['srlz_cls', type(self.child).__qualname__,]
        if not isinstance(value, (int,float)):
            err_msg = 'Quota value is %s, which is neither integer or float number' % value
            log_msg.extend(['err_msg', err_msg])
            _logger.info(None, *log_msg)
            raise ValueError(err_msg)
        self._applied_quota = value
        self.edit_quota_threshold(quota_validator=self._quota_validator, value=value)
        log_msg.extend(['_applied_quota', self._applied_quota])
        _logger.debug(None, *log_msg)

    def edit_quota_threshold(self, quota_validator, value):
        raise NotImplementedError()

