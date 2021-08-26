import json
import logging

from django.conf   import settings
from django.db     import models
from django.utils  import timezone
from django.core.exceptions  import ObjectDoesNotExist

from django.contrib.contenttypes.models  import ContentType
from django.contrib.contenttypes.fields  import GenericForeignKey

_logger = logging.getLogger(__name__)

class _SoftDeleteIdSerializerMixin:
    """
    To support soft-deleted instance with compound primary key (multi-column pkey)
    , the pk value can be JSON-compilant, serializable list or dict. This mixin
    declare serialize / deserialize methods to interact with soft-delete models.
    """
    @classmethod
    def serialize_obj_id(cls, value):
        if isinstance(value, (int, float)):
            out = str(value)
        else:
            out = json.dumps(value, sort_keys=True)
        return out

    @classmethod
    def deserialize_obj_id(cls, value):
        if isinstance(value, (int,)):
            out = int(value)
        elif isinstance(value, (float,)):
            out = float(value)
        else:
            out = json.loads(value) # TODO, ensure key order
        return out

    @property
    def deserialized_obj_id(self):
        return type(self).deserialize_obj_id(self.object_id)


class ChangeSet(models.Model, _SoftDeleteIdSerializerMixin):
    class Meta:
        abstract = True
    ##done_by  = models.ForeignKey('user_management.GenericUserProfile', db_column='done_by', null=True,
    ##            on_delete=models.SET_NULL, related_name="softdel_cset")
    done_by = models.CharField(max_length=16, null=False, blank=False, unique=False)
    time_created = models.DateTimeField(default=timezone.now)
    content_type = models.ForeignKey(ContentType, db_column='content_type', on_delete=models.CASCADE)
    # currently valid data type of `object_id` could be integer or serializable dictionary or list,
    # all other data types will result in changeset lookup error. The same sulr is also applied
    # to SoftDeleteRecord declared below
    object_id = models.CharField(db_column='object_id', max_length=100)
    record    = GenericForeignKey(ct_field='content_type', fk_field='object_id')

    @classmethod
    def foreignkey_fieldtype(cls):
        assert issubclass(cls, ChangeSet), 'cls must be ChangeSet'
        return  models.ForeignKey(cls, db_column='changeset', related_name='soft_delete_records',
                on_delete=models.CASCADE)


class SoftDeleteRecord(models.Model, _SoftDeleteIdSerializerMixin):
    class Meta:
        abstract = True
        unique_together = (('changeset','content_type','object_id'),)
    time_created = models.DateTimeField(default=timezone.now)
    content_type = models.ForeignKey(ContentType, db_column='content_type', on_delete=models.CASCADE)
    object_id = models.CharField(db_column='object_id', max_length=100)
    record    = GenericForeignKey(ct_field='content_type', fk_field='object_id')



class SoftDeleteQuerySet(models.QuerySet):
    # TODO, ensure atomicity
    def delete(self, *args, **kwargs):
        for obj in self:
            obj.delete(*args, **kwargs)

    def undelete(self, *args, **kwargs):
        for obj in self:
            obj.undelete(*args, **kwargs)


class SoftDeleteManager(models.Manager):
    default_qset_cls = None

    def _get_base_queryset(self):
        return super(SoftDeleteManager, self).get_queryset()

    def _get_self_queryset(self):
        return self.get_queryset()

    def get_queryset(self):
        qs = self._get_base_queryset().filter(time_deleted__isnull=True)
        qs.__class__ = self.default_qset_cls or  SoftDeleteQuerySet
        return qs

    def get_deleted_set(self):
        qs = self._get_base_queryset().filter(time_deleted__isnull=False)
        qs.__class__ = self.default_qset_cls or  SoftDeleteQuerySet
        return qs

    def all(self, with_deleted=False):
        """ get queryset for all objects in the model, regardless the delete status of each object """
        if with_deleted:
            qs = self._get_base_queryset()
            if hasattr(self, 'core_filters'): # argument to RelatedManager
                qs = qs.filter(**self.core_filters)
            # Originally the class is django queryset, but it has to be soft-delete specific 
            # queryset to caller
            qs.__class__ = self.default_qset_cls or  SoftDeleteQuerySet
        else:
            qs = super(SoftDeleteManager, self).all()
        return qs

    def _get_qset_common(self, pk=None):
        if pk:
            return self.all(with_deleted=True)
        else:
            return self._get_self_queryset()

    def get(self, *args, **kwargs):
        return self._get_qset_common(kwargs.get('pk',None)).get(*args, **kwargs)

    def filter(self, *args, **kwargs):
        with_deleted = kwargs.pop('with_deleted', False)
        if with_deleted:
            qs = self.all(with_deleted=True).filter(*args, **kwargs)
        else:
            qs = self._get_qset_common(kwargs.get('pk',None)).filter(*args, **kwargs)
        qs.__class__ = self.default_qset_cls or  SoftDeleteQuerySet
        return qs



class SoftDeleteObjectMixin(models.Model):
    class Meta:
        abstract = True

    RECOVERY_DO_NOTHING   = 1
    DONE_PARTIAL_RECOVERY = 2
    DONE_FULL_RECOVERY    = 3

    SOFTDELETE_CHANGESET_MODEL = None
    SOFTDELETE_RECORD_MODEL = None

    objects = SoftDeleteManager()
    time_deleted = models.DateTimeField(blank=True, null=True, default=None, editable=False) # db_index=True


    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._time_deleted_origin = self.time_deleted

    def is_deleted(self):
        return self.time_deleted  is not None

    def save(self, force_insert=False, force_update=False, using=None, update_fields=None, enable_logging=False):
        time_deleted_before = self._time_deleted_origin
        time_deleted_after = self.time_deleted
        self._edit_flag = (self.id is not None) and (time_deleted_before is None) and (time_deleted_after is None)
        self._insert_flag = self.id is None
        super().save(force_insert=force_insert, force_update=force_update,
                using=using,  update_fields=update_fields)
        if enable_logging is True:
            log_args = ['time_deleted_before', time_deleted_before, 'time_deleted_after', time_deleted_after,
                    'edit_flag', self._edit_flag, 'insert_flag', self._insert_flag, 'model_cls', type(self),
                    'self.pk', self.pk]
            _logger.debug(None, *log_args)


    def _delete_relations(self, related_fields, *args, **kwargs):
        softdel_model_types = tuple([SoftDeleteObjectMixin, SoftDeleteQuerySet])
        skip_model_types = [ChangeSet, SoftDeleteRecord]
        skip_model_types.extend(kwargs.pop('skip_model_types', []))
        skip_model_types = tuple(skip_model_types)
        log_args = []
        loglevel = logging.DEBUG
        for f in related_fields:
            rel_attr = f.get_accessor_name()
            if not hasattr(self, rel_attr): # some fields exist in the model instance but not accessible
                continue
            rel = getattr(self, rel_attr)
            if not f.one_to_one: # rel is either Model or QuerySet instance
                rel = rel.all()

            if f.one_to_one:
                if isinstance(rel, skip_model_types) or (rel.pk is None):
                    skip_log = {'rel_attr':rel_attr, 'one_to_one':True, 'rel_cls': type(rel), 'rel_id': rel.pk,}
                    log_args.extend(['skip_related_field', skip_log])
                    loglevel = logging.INFO
                    continue
            else: # must be queryset
                if rel.model in skip_model_types:
                    skip_log = {'rel_attr':rel_attr, 'one_to_one':False, 'rel_cls': type(rel)}
                    log_args.extend(['skip_related_field', skip_log])
                    loglevel = logging.INFO
                    continue

            del_rel_log = {'rel_attr':rel_attr, 'rel': rel}
            log_args.extend(['deleting_related_field', del_rel_log])
            changeset_bak = None
            if not isinstance(rel, softdel_model_types):
                changeset_bak = kwargs.pop('changeset', None)
            rel.delete(*args, **kwargs)
            if not isinstance(rel, softdel_model_types):
                kwargs['changeset'] = changeset_bak
        if any(log_args):
            log_args.extend(['model_cls', type(self), 'model_id', self.pk, 'changeset_id',
                kwargs['changeset'].pk])
            _logger.log(loglevel, None, *log_args)


    def delete(self, *args, **kwargs):
        """
        perform soft-delete first (unless specified as hard-delete), let application caller decides when to move out
        (dump) data from live database table to another storage (e.g. archive DB table, files, ...etc)
        by calling xxx() TODO
        """
        hard = kwargs.get('hard', False) # is it hard-delete ?
        if hard:
            del kwargs['hard']
            changeset = kwargs.pop('changeset', None)
            super(SoftDeleteObjectMixin, self).delete(*args, **kwargs)
            if not changeset is None:
                kwargs['changeset'] = changeset
        elif not self.is_deleted():
            profile_id = kwargs.pop('profile_id',None)
            cs = kwargs.get('changeset', None) or self.determine_change_set(profile_id=profile_id)
            kwargs['changeset'] = cs
            self.SOFTDELETE_RECORD_MODEL.objects.get_or_create( changeset=cs,
                    object_id=self.SOFTDELETE_RECORD_MODEL.serialize_obj_id(value=self.pk),
                    content_type=ContentType.objects.get_for_model(self) )
            self.time_deleted = timezone.now()
            self.save()
            # TODO, provide extra instance variable for subclasses which determines
            #      what related fields to delete followed by the instance deletion.
            related_fields = [f for f in self._meta.get_fields() if f.auto_created and
                                not f.concrete and (f.one_to_many or f.one_to_one)]
            self._delete_relations(related_fields=related_fields, *args, **kwargs)
            if profile_id:
                softdel_recs = cs.soft_delete_records.values_list('pk', flat=True)
                log_args = ['model_cls', type(self), 'model_id', self.pk, 'changeset_id', cs.pk,
                        'profile_id', profile_id, 'softdel_recs', softdel_recs]
                _logger.debug(None, *log_args)


    def undelete(self, *args, **kwargs):
        if not self.is_deleted():
            return self.RECOVERY_DO_NOTHING
        profile_id = kwargs.pop('profile_id', None)
        cs = kwargs.get('changeset', None) or self.determine_change_set(profile_id=profile_id, create=False)
        related_records = cs.soft_delete_records.all()
        filtered_fields = self.filter_before_recover(records_in=related_records)
        for f in  filtered_fields:
            model_cls = f.content_type.model_class()
            rel = model_cls.objects.get(pk=f.deserialized_obj_id)
            rel.time_deleted = None
            rel.save()
        changeset_id =  cs.pk
        related_records_id = [d.pk for d in related_records]
        filtered_fields_id = [d.pk for d in filtered_fields]
        cs.delete()
        discarded = set(related_records) - set(filtered_fields)
        discarded_id = [d.pk for d in discarded]
        log_args = ['model_cls', type(self), 'model_id', self.pk, 'changeset_id', changeset_id,
                'related_records_id', related_records_id, 'filtered_fields_id', filtered_fields_id, 'discarded_id', discarded_id]
        _logger.debug(None, *log_args)
        for d in discarded:
            model_cls = d.content_type.model_class() # model_cls must inherit SoftDeleteObjectMixin
            obj = model_cls.objects.get(pk=d.object_id)
            obj.delete(hard=True)
        return self.DONE_PARTIAL_RECOVERY if any(discarded) else self.DONE_FULL_RECOVERY


    def determine_change_set(self, profile_id, create=True):
        content_type = ContentType.objects.get_for_model(self)
        qs = None
        log_args = ['model_cls', type(self), 'model_id', self.pk, 'profile_id', profile_id]
        loglevel = logging.DEBUG
        try:
            serialized_object_id = self.SOFTDELETE_RECORD_MODEL.serialize_obj_id(value=self.pk)
            qs = self.SOFTDELETE_RECORD_MODEL.objects.filter(content_type=content_type,
                    object_id=serialized_object_id,  changeset__done_by=profile_id )
            qs = qs.latest('time_created').changeset
            log_args.extend(['msg', 'Found changeSet via latest recordset.'])
        except self.SOFTDELETE_RECORD_MODEL.DoesNotExist:
            serialized_object_id = self.SOFTDELETE_CHANGESET_MODEL.serialize_obj_id(value=self.pk)
            try:
                qs = self.SOFTDELETE_CHANGESET_MODEL.objects.filter(content_type=content_type,
                        object_id=serialized_object_id, done_by=profile_id)
                qs = qs.latest('time_created')
                log_args.extend(['msg', 'Found changeSet via changeset'])
            except self.SOFTDELETE_CHANGESET_MODEL.DoesNotExist:
                if create:
                    qs = self.SOFTDELETE_CHANGESET_MODEL.objects.create(content_type=content_type,
                            object_id=serialized_object_id,  done_by=profile_id)
                    log_args.extend(['msg', 'new changeSet created'])
                    loglevel = logging.INFO
                else:
                    err_msg = 'changeset not found and NOT allowed to create new one'
                    log_args.extend(['msg', err_msg])
                    _logger.error(None, *log_args)
                    raise ObjectDoesNotExist(err_msg)
        log_args.extend(['changeset_id', qs.pk])
        _logger.log(loglevel, None, *log_args)
        return qs


    def filter_before_recover(self, records_in):
        """
        subclasses can override this function to filter out some useless records
        for apllication-level recovery (e.g. to avoid data corruption after recovery)
        """
        return records_in



