from functools  import wraps
import logging

from django.db      import models
from django.contrib.contenttypes.models  import ContentType

_logger = logging.getLogger(__name__)

def get_paths_through_processing_node(with_deleted=False):
    def inner(process_fn):
        @wraps(process_fn)
        def wrapper(self, *args, **kwargs):
            descs_qset = self.descendants.filter(with_deleted=with_deleted, depth__gt=0)
            descs_qset = descs_qset.values_list('descendant', flat=True)
            ascs_qset  = self.ancestors.filter(with_deleted=with_deleted, depth__gt=0)
            ascs_qset  = ascs_qset.values_list('ancestor', flat=True)
            path_cls = self.ancestors.model
            condition = models.Q(descendant__in=descs_qset) & models.Q(ancestor__in=ascs_qset)
            affected_paths = path_cls.objects.filter(condition, with_deleted=with_deleted)
            process_fn(self, affected_paths=affected_paths)
            return affected_paths
        return wrapper
    return inner

#all_descendants = self.descendants.filter(with_deleted=with_deleted, depth__gt=0)
#all_ancestors   = self.ancestors.filter(with_deleted=with_deleted, depth__gt=0)
#search_ancestor = [a.ancestor.pk  for a in all_ancestors]
#for d in all_descendants:
#    for a in d.descendant.ancestors.all(with_deleted=with_deleted):
#        if a.ancestor.pk in search_ancestor:
#            kwargs['closure_path'] = a
#            process_fn(self, *args, **kwargs)
#            kwargs.pop('closure_path',None)


def filter_closure_nodes_recovery(records_in, app_label, model_name):
    """
    check whether a soft-deleted node can be undeleted and recovered to the closure table tree.
    Input `records_in` is a SoftDeleteRecord instance that contains the soft-deleted node and
    associated paths from/to the node, this function should determine whether all these
    soft-deleted paths can be recovered or not.
    """
    if not any(records_in) :
        return records_in
    record  = records_in.first() if isinstance(records_in, models.QuerySet) else records_in[0]
    node_ct = record.changeset.content_type
    node_cls = node_ct.model_class()
    del_node = node_cls.objects.get(pk=record.changeset.object_id)
    path_ct = ContentType.objects.get(app_label=app_label , model=model_name)
    ####cset_path_ids = [r.object_id for r in records_in.filter(content_type=path_ct.pk)]
    cset_path_ids = records_in.filter(content_type=path_ct.pk).values_list('object_id', flat=True)
    check_failed = False
    # (1). check whether all the ancestors are at the same position as they were (if exists before this node was deleted)
    del_asc  = del_node.ancestors.filter(with_deleted=True, depth__gt=0, pk__in=cset_path_ids).order_by('depth')
    del_desc = del_node.descendants.filter(with_deleted=True, depth__gt=0, pk__in=cset_path_ids)
    log_msg = ['node_cls', node_cls, 'app_label', app_label, 'model_name', model_name,
            'path_ct', path_ct, 'del_node', del_node.pk]
    loglevel = logging.DEBUG
    if del_asc.exists() : #any(del_asc)
        del_asc_0 = del_asc.first()
        if del_asc_0.depth != 1:  ####assert del_asc[0].depth == 1, ""
            err_msg = "There must be at least one parent immediately connected to this node"
            log_msg.extend(['del_asc_0.ancestor', del_asc_0.ancestor.pk, 'del_asc_0.descendant', del_asc_0.descendant.pk,
                'del_asc_0.depth', del_asc_0.depth, 'err_msg', err_msg])
            _logger.error(None, *log_msg)
            raise AssertionError(err_msg)
        parent = del_asc_0.ancestor
        new_asc = parent.ancestors.all()
        del_asc_set = [(a.ancestor.pk, a.depth)     for a in del_asc]
        new_asc_set = [(a.ancestor.pk, a.depth + 1) for a in new_asc]
        diff = list(set(del_asc_set).symmetric_difference(new_asc_set))
        log_msg.extend(['del_asc_0.ancestor', parent.pk, 'del_asc_set', del_asc_set, 'new_asc_set', new_asc_set])
        if any(diff):
            check_failed = True
            log_msg.extend(['asc_check_failed', check_failed, 'asc_diff', diff])
            loglevel = logging.WARNING
    else:
        parent = None

    # (2). check whether all the descendants are as it were (if exists before this node was deleted)
    if not check_failed and del_desc.exists(): #any(del_desc)
        children = del_desc.filter(depth=1)
        if children.exists() is False:  ####assert any(children),""
            err_msg = "There must be at least one child nodes among all the descendants"
            log_msg.extend(['err_msg', err_msg])
            _logger.error(None, *log_msg)
            raise AssertionError(err_msg)
        new_desc = [d  for c in children  for d in c.descendant.descendants.all()] # flatten 2D list
        del_desc_set = [(d.descendant.pk, d.depth)     for d in del_desc]
        new_desc_set = [(d.descendant.pk, d.depth + 1) for d in new_desc]
        diff = list(set(del_desc_set).symmetric_difference(new_desc_set))
        log_msg.extend(['del_desc_set', del_desc_set, 'new_desc_set', new_desc_set])
        if any(diff):
            check_failed = True
            log_msg.extend(['desc_check_failed', check_failed, 'desc_diff', diff, 'children', children])
            loglevel = logging.WARNING
        # (3). check whether all the child nodes of the deleted node (immediately connected to it)
        # are still connected to the parent of the deleted node.
        if parent and not check_failed:
            new_children = parent.descendants.filter(with_deleted=False, depth=1)
            del_child_set = children.values_list('descendant__pk', flat=True)     #### [c.descendant.pk for c in children]
            new_child_set = new_children.values_list('descendant__pk', flat=True) #### [c.descendant.pk for c in new_children]
            diff = list(set(del_child_set) - set(new_child_set))
            log_msg.extend(['del_child_set', del_child_set, 'new_child_set', new_child_set])
            if any(diff):
                check_failed = True
                log_msg.extend(['tree_rel_check_failed', check_failed, 'tree_rel_diff', diff])
                loglevel = logging.WARNING

    if check_failed: # recovery failure, the deleted closure table paths in this changeset must be discarded
        exclude_path_ids = [a.pk for a in del_asc] + [d.pk for d in del_desc]
        records_out = records_in.exclude(content_type=path_ct.pk , object_id__in=exclude_path_ids)
    else:
        log_msg.extend(['check_failed', check_failed])
        records_out = records_in
    _logger.log(loglevel, None, *log_msg)
    return records_out
### end of filter_closure_nodes_recovery()


class ClosureTableModelMixin(models.Model):
    """
    subclasses must implement 2 fields with the exact naming `ancestor` and `descendant`
    """
    class Meta:
        abstract = True
        constraints = [models.UniqueConstraint(fields=['ancestor','descendant'], name="unique_path",)]

    depth = models.PositiveIntegerField(db_column='depth', default=0)

    def save(self, *args, accept_null_node=False, **kwargs):
        if accept_null_node is False:
            asc  = getattr(self, 'ancestor', None)
            desc = getattr(self, 'descendant', None)
            if asc is None or desc is None:
                log_msg = ['accept_null_node', accept_null_node, 'asc', asc, 'desc', desc, 'node_pk', self.pk]
                _logger.warning(None, *log_msg)
                raise ValueError('Null closure node not allowed')
        return super().save(*args, **kwargs)

    @classmethod
    def asc_field(cls, ref_cls):
        return models.ForeignKey(ref_cls , db_column='ancestor', null=True,
                        on_delete=models.CASCADE, related_name='descendants')

    @classmethod
    def desc_field(cls, ref_cls):
        return models.ForeignKey(ref_cls , db_column='descendant', null=True,
                        on_delete=models.CASCADE, related_name='ancestors')



