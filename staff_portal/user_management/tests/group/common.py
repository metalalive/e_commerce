import random
import copy
import operator
from functools import reduce

from django.utils  import timezone as django_timezone
from django.db.models.constants import LOOKUP_SEP

from user_management.models.common import AppCodeOptions
from user_management.models.auth import Role
from user_management.models.base import QuotaMaterial, GenericUserProfile, GenericUserGroup, GenericUserGroupClosure, EmailAddress, PhoneNumber, GeoLocation
from user_management.serializers import GenericUserGroupSerializer

from tests.python.common import HttpRequestDataGen
from ..common import _fixtures, _curr_timezone, gen_expiry_time, UserNestedFieldSetupMixin, UserNestedFieldVerificationMixin

_nested_field_names = {
    'roles': ['expiry', 'role'],
    'quota': ['material','maxnum','expiry'],
    'locations':['id', 'country', 'province', 'locality', 'street', 'detail', 'floor', 'description'],
    'emails':['id','addr'],
    'phones':['id','line_number','country_code'],
}


def _auto_increment_gen_fn(num=0):
    while True:
        num = num + 1
        yield num

_auto_inc_gen =  _auto_increment_gen_fn()


class HttpRequestDataGenGroup(HttpRequestDataGen, UserNestedFieldSetupMixin):
    def init_primitive(self):
        keys = (Role, QuotaMaterial, GenericUserProfile)
        data_map = dict(map(lambda cls: (cls, _fixtures[cls]), keys))
        objs = {k_cls: list(map(lambda d: k_cls(**d), data)) for k_cls, data in data_map.items()}
        for cls in keys:
            cls.objects.bulk_create(objs[cls])
        self._primitives = objs
        return objs

    def _gen_roles(self, num=None):
        return super()._gen_roles(role_objs=self._primitives[Role] , num=num)

    def _gen_quota(self, num=None):
        return super()._gen_quota(quota_mat_objs=self._primitives[QuotaMaterial], num=num)

    def _gen_name(self):
        num_valid_grps = len(_fixtures[GenericUserGroup])
        idx = random.randrange(0, num_valid_grps)
        return  _fixtures[GenericUserGroup][idx]['name']

    def _write_value_fn(self, node):
        # this function has to ensure that _gen_quota() runs prior to other functions
        # which generate emails, locations, phone-numbers
        out = {fname: getattr(self, '_gen_%s' % fname)()  for fname in _nested_field_names.keys()}
        out['name'] = self._gen_name()
        node.value = out

    def trees_to_req_data(self, trees, shuffle=False):
        out = []
        for root in trees:
            req_data = self._tree_to_req_data(curr_node=root, parent_data=None)
            out.extend(req_data)
        if shuffle:
            random.shuffle(out)
        for d in out:
            if d['new_parent'] is not None:
                idx = out.index(d['new_parent'])
                d['new_parent'] = idx
        for d in out:
            d.pop('_unik_key', None)
        return out

    def _tree_to_req_data(self, curr_node, parent_data):
        req_data = copy.deepcopy(curr_node.value)
        req_data['new_parent'] = parent_data
        req_data['_unik_key'] = next(_auto_inc_gen)
        out = [req_data]
        for child in curr_node.children:
            child_req_data = self._tree_to_req_data(curr_node=child, parent_data=req_data)
            out.extend(child_req_data)
        return out

    def _moving_nodes_to_req_data(self, moving_nodes):
        field_names = tuple(_nested_field_names.keys()) + ('id', 'name',)
        req_data = []
        for node in moving_nodes:
            data = {fname: node.value[fname] for fname in field_names}
            data['exist_parent'] = node.parent.value['id'] if node.parent else None
            data['new_parent'] = None
            req_data.append(data)
        random.shuffle(req_data) # `id` field should be unique value in each data item
        return req_data
## end of class HttpRequestDataGenGroup


class GroupVerificationMixin(UserNestedFieldVerificationMixin):
    serializer_class = GenericUserGroupSerializer
    err_msg_loop_detected = 'will form a loop, which is NOT allowed in closure table'
    _nested_field_names = _nested_field_names

    def load_closure_data(self, node_ids):
        # load closure data from django ORM, not from DRF serializer because
        # serializer gets rid of unecessary data like closure id
        closure_node_cls = GenericUserGroupClosure
        entity_cls = self.serializer_class.Meta.model
        condition_kwargs = {LOOKUP_SEP.join(['descendant','id','in']): node_ids}
        closure_qset = closure_node_cls.objects.filter(**condition_kwargs).values(
                'id', 'ancestor', 'descendant', 'depth')
        entity_qset = entity_cls.objects.filter(id__in=node_ids)
        return entity_qset, closure_qset

    def _closure_node_value_setup(self, node):
        out = self.load_group_from_instance(obj=node)
        out['name'] = node.name
        return out

    def _value_compare_fn(self, val_a, val_b):
        fields_eq = super()._value_compare_fn(val_a, val_b)
        fields_eq['name'] = val_a['name'] == val_b['name']
        return reduce(operator.and_, fields_eq.values())


