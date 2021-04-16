from datetime import datetime, timezone, timedelta
import logging

from django.core.exceptions  import ValidationError
from django.core.validators  import EMPTY_VALUES

from rest_framework             import status as RestStatus
from rest_framework.response    import Response as RestResponse
from rest_framework.settings    import api_settings

_logger = logging.getLogger(__name__)


class RecoveryModelMixin:
    # this mixin class has to work with any subclass of 
    # common.views.proxy.mixins.BaseGetProfileIDMixin
    MAX_TIME_DELTA = timedelta(**{'days': 10, 'hours':12,})
    ZERO_TIME_DELTA = timedelta()
    TIME_UNIT = ['year', 'month', 'day', 'hour', 'minute']
    SOFTDELETE_CHANGESET_MODEL = None # subclasses must override this variable

    def _get_time_range(self, first_cset, time_start=''):
        # TODO: find better way to find the items that were deleted together
        log_args = []
        delta_end = first_cset.time_created + timedelta(milliseconds=1)
        if time_start in EMPTY_VALUES: # only recover last deletion
            delta_start = delta_end - timedelta(seconds=3)
            log_args.extend(['delta_start',delta_start,'delta_end',delta_end])
        else:
            clean_time_start = time_start.split("-")[:len(self.TIME_UNIT)]
            try: # the acceptable format is YYYY-MM-DD-hh-mm
                clean_time_start = [int(d) for d in clean_time_start]
                clean_time_start = {self.TIME_UNIT[idx]: clean_time_start[idx] for idx in range(len(self.TIME_UNIT))}
                clean_time_start['tzinfo'] = timezone.utc
                delta_start = datetime(**clean_time_start)
                diff = delta_end - delta_start
                log_args.extend(['delta_start',delta_start,'delta_end',delta_end, 'diff', diff,
                    'clean_time_start', clean_time_start])
                # report error if the requested range is too large and negative value
                if diff < self.ZERO_TIME_DELTA: # negative delta
                    errmsg = "the specified starting time must be in the past"
                    raise ValueError(errmsg)
                elif diff > self.MAX_TIME_DELTA:
                    errmsg = "the range must be less than {td}".format(td=str(self.MAX_TIME_DELTA))
                    raise ValueError(errmsg)
            except (ValueError, IndexError, TypeError) as e:
                if isinstance(e, IndexError):
                    errmsg = "the format should be YYYY-MM-DD-hh-mm"
                else:
                    errmsg = str(e)
                msg = "invalid start time {tf}, {errmsg}".format(tf=time_start, errmsg=errmsg)
                log_args.extend(['msg', msg])
                _logger.info(None, *log_args, request=self.request)
                raise ValidationError(message=msg,  params={'status': RestStatus.HTTP_400_BAD_REQUEST},)
        _logger.debug(None, *log_args, request=self.request)
        return  delta_start, delta_end


    def recovery(self, request, *args, **kwargs):
        # check whether to apply recovery view from soft-delete module
        status = kwargs.pop('status_ok', RestStatus.HTTP_200_OK)
        return_params = {}
        resource_content_type = kwargs.pop('resource_content_type', None)
        loglevel = logging.INFO
        log_args = ['action', 'recover', 'resource_content_type', resource_content_type]
        if resource_content_type:
            profile_id = self.get_profile_id(request=request)
            body = request.data
            m_cls = resource_content_type.model_class()
            model_cls_hier = "%s.%s" % (m_cls.__module__ , m_cls.__qualname__)
            log_args.extend(['profile_id', profile_id,'model_cls', model_cls_hier])
            first_cset = self.SOFTDELETE_CHANGESET_MODEL.objects.filter(content_type=resource_content_type.pk, done_by=profile_id
                    ).order_by("-time_created").first()

            if first_cset:
                # add callback accessibility because deletion is NOT handled by serializer
                failure_callback = getattr(self, 'recover_failure_callback', None)
                success_callback = getattr(self, 'recover_success_callback', None)
                _id_list = []
                try:
                    delta_start, delta_end = self._get_time_range(first_cset=first_cset, time_start=body.get('time_start', ''))
                    cset = self.SOFTDELETE_CHANGESET_MODEL.objects.filter(content_type=resource_content_type.pk, time_created__lt=delta_end, \
                            done_by=profile_id, time_created__gt=delta_start).order_by("-time_created")
                    result = []
                    content_type_ids = []
                    for c in cset:   # recover items in opposite order in which they were deleted
                        _id_list.append(c.object_id)
                        content_type_ids.append('pk=%s, content_type=%s, object_id=%s' %
                                (c.pk, c.content_type.pk, c.object_id))
                        m_obj = m_cls.objects.get(pk=c.object_id)
                        m_obj.undelete(changeset=c, profile_id=profile_id)
                        result.append( m_obj.minimum_info )
                    log_args.extend(['affected_items', result, 'content_type_ids', content_type_ids])
                    if success_callback:
                        success_callback(id_list=_id_list)
                    return_msg = "recovery done"
                    return_params['recovered'] = result
                except ValidationError as e:
                    if failure_callback:
                        failure_callback(err=e, id_list=_id_list)
                    status = e.params.get('status', status)
                    return_msg = str(e.message)
                    loglevel = logging.WARNING
            else:
                # this function works only when changeSet has something to recover, if it is empty
                # , it would be great to indicate that nothing can be processed and deleted from
                # changeSet, by sending response status 410
                status = RestStatus.HTTP_410_GONE
                return_msg = "Nothing recovered"
        else:
            return_msg = "caller must specify resource content type"
            status = RestStatus.HTTP_500_INTERNAL_SERVER_ERROR
            loglevel = logging.WARNING

        log_args.extend(['return_msg', return_msg, 'http_status', status])
        _logger.log(loglevel, None, *log_args, request=request)
        return_data = {'message':[return_msg], 'params':return_params}
        return RestResponse(return_data, status=status)

