from datetime import datetime, timezone, timedelta
import logging

from django.core.exceptions  import ValidationError, ObjectDoesNotExist
from django.core.validators  import EMPTY_VALUES

from rest_framework             import status as RestStatus
from rest_framework.response    import Response as RestResponse
from rest_framework.exceptions  import ParseError
from rest_framework.settings    import api_settings

_logger = logging.getLogger(__name__)


class RecoveryModelMixin:
    MAX_TIME_DELTA = timedelta(**{'days': 10, 'hours':12,})
    MIN_TIME_DELTA = timedelta(**{'seconds': 2,})
    ZERO_TIME_DELTA = timedelta()
    TIME_UNIT = ['year', 'month', 'day', 'hour', 'minute']
    SOFTDELETE_CHANGESET_MODEL = None # subclasses must override this variable

    def _get_time_range(self, first_cset, time_start=''):
        # TODO: find better way to find the items that were deleted together
        log_args = []
        delta_end = first_cset.time_created + timedelta(milliseconds=1)
        if time_start in EMPTY_VALUES: # only recover last deletion
            delta_start = delta_end - self.MIN_TIME_DELTA
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


    def recovery(self, request, profile_id, *args, status_ok=RestStatus.HTTP_200_OK, resource_content_type=None,
            return_data_after_done=False, **kwargs):
        # check whether to apply recovery view from soft-delete module
        status = status_ok
        affected_items = None
        loglevel = logging.INFO
        log_args = ['action', 'recover', 'resource_content_type', resource_content_type]
        if resource_content_type:
            m_cls = resource_content_type.model_class()
            model_cls_hier = "%s.%s" % (m_cls.__module__ , m_cls.__qualname__)
            log_args.extend(['profile_id', profile_id,'model_cls', model_cls_hier])
            try:
                body = request.data or {}
            except ParseError as e:
                body = {}
            if isinstance(body, dict):
                ids = body.get('ids', [])
                time_start = body.get('time_start', '')
                if any(ids):
                    status, return_msg,  affected_items, loglevel = self._recover_by_id(
                        model_cls=m_cls, ids=ids,  profile_id=profile_id, status_ok=status_ok,
                        return_data_after_done=return_data_after_done, log_args=log_args)
                else:
                    status, return_msg,  affected_items, loglevel = self._recover_by_newest_deletion(
                        time_start=time_start, profile_id=profile_id, model_cls=m_cls,
                        resource_content_type=resource_content_type, log_args=log_args,
                        return_data_after_done=return_data_after_done, status_ok=status_ok )
            else:
                return_msg = "invalid data in request body"
                status = RestStatus.HTTP_400_BAD_REQUEST
                loglevel = logging.WARNING
        else:
            return_msg = "caller must specify resource content type"
            status = RestStatus.HTTP_500_INTERNAL_SERVER_ERROR
            loglevel = logging.WARNING

        log_args.extend(['return_msg', return_msg, 'http_status', status])
        _logger.log(loglevel, None, *log_args, request=request)
        response_data = {'message':[return_msg],}
        if affected_items:
            response_data['affected_items'] = affected_items
        return RestResponse(response_data, status=status)
    ## end of recovery()

    def _recover_by_newest_deletion(self, model_cls, resource_content_type, time_start,
            profile_id, status_ok, return_data_after_done, log_args):
        status = status_ok
        loglevel = logging.INFO
        affected_items = None
        first_cset = self.SOFTDELETE_CHANGESET_MODEL.objects.filter(content_type=resource_content_type.pk,
                done_by=profile_id ).order_by("-time_created").first()
        if first_cset:
            # add callback accessibility because deletion is NOT handled by serializer
            failure_callback = getattr(self, 'recover_failure_callback', None)
            success_callback = getattr(self, 'recover_success_callback', None)
            _id_list = []
            try:
                delta_start, delta_end = self._get_time_range(first_cset=first_cset, time_start=time_start)
                # recover items in opposite order in which they were deleted
                cset = self.SOFTDELETE_CHANGESET_MODEL.objects.filter(content_type=resource_content_type.pk,
                        time_created__lt=delta_end, done_by=profile_id, time_created__gt=delta_start
                        ).order_by("-time_created")
                _id_list = cset.values_list('object_id', flat=True)
                _id_list = list(_id_list) # do not do this after un-delete the instances
                m_objs = model_cls.objects.filter(pk__in=_id_list , with_deleted=True)
                content_type_ids = cset.values_list('pk','content_type','object_id')
                m_objs.undelete(profile_id=profile_id)
                log_affected_items = [m_obj.minimum_info for m_obj in m_objs]
                log_args.extend(['affected_items', log_affected_items, 'content_type_ids', content_type_ids])
                if return_data_after_done:
                    _serializer =  self.get_serializer(many=True, instance=m_objs)
                    affected_items = _serializer.data
                if success_callback:
                    success_callback(id_list=_id_list)
                return_msg = "recovery done"
            except ValidationError as e:
                if failure_callback:
                    failure_callback(err=e, id_list=_id_list)
                status = RestStatus.HTTP_400_BAD_REQUEST ## e.params.get('status', status)
                return_msg = str(e.message)
                loglevel = logging.WARNING
        else:
            # this function works only when changeSet has something to recover, if it is empty
            # , it would be great to indicate that nothing can be processed and deleted from
            # changeSet, by sending response status 410
            status = RestStatus.HTTP_410_GONE
            return_msg = "Nothing recovered"
        return (status, return_msg,  affected_items, loglevel)


    def _recover_by_id(self, model_cls, ids, profile_id, status_ok, return_data_after_done, log_args):
        status = status_ok
        loglevel = logging.INFO
        affected_items = None
        failure_callback = getattr(self, 'recover_failure_callback', None)
        success_callback = getattr(self, 'recover_success_callback', None)
        try: # input ID list replies on permission check before getting into this view
            m_objs = model_cls.objects.get_deleted_set().filter(pk__in=ids)
        except ValueError as e:
            m_objs = model_cls.objects.none()
            status = RestStatus.HTTP_400_BAD_REQUEST
            return_msg = "invalid data in request body"
            if failure_callback and callable(failure_callback):
                failure_callback(err=e, id_list=ids)
        if m_objs.exists():
            try:
                m_objs.undelete(profile_id=profile_id)
                log_affected_items = [m_obj.minimum_info for m_obj in m_objs]
                log_args.extend(['affected_items', log_affected_items,])
                if return_data_after_done:
                    _serializer =  self.get_serializer(many=True, instance=m_objs)
                    affected_items = _serializer.data
                if success_callback and callable(success_callback):
                    success_callback(id_list=ids)
                return_msg = "recovery done"
            except ObjectDoesNotExist as e: # changeset not found
                err_msg = ','.join(e.args)
                if failure_callback:
                    failure_callback(err=e, id_list=ids)
                if 'changeset not found' in err_msg:
                    status = RestStatus.HTTP_403_FORBIDDEN
                    return_msg = "user is not allowed to undelete the item(s)"
                else:
                    raise
        elif status == status_ok: # ensure there is no soft-deleted item
            status = RestStatus.HTTP_410_GONE
            return_msg = "Nothing recovered"
        return (status, return_msg,  affected_items, loglevel)
## end of class RecoveryModelMixin


