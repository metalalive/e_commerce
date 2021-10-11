import random
import copy
import operator
from functools import reduce
from datetime import timedelta

from django.utils  import timezone
from django.db.models.constants import LOOKUP_SEP

from user_management.models.common import AppCodeOptions
from user_management.models.auth import Role
from user_management.models.base import QuotaMaterial, GenericUserProfile, GenericUserGroup, GenericUserGroupClosure, EmailAddress, PhoneNumber, GeoLocation
from user_management.serializers import GenericUserGroupSerializer

from tests.python.common import rand_gen_request_body, listitem_rand_assigner, HttpRequestDataGen
from ..common import _fixtures

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


class HttpRequestDataGenGroup(HttpRequestDataGen):
    num_roles = 0
    num_quota = 0

    def init_primitive(self):
        keys = (Role, QuotaMaterial, GenericUserProfile)
        data_map = dict(map(lambda cls: (cls, _fixtures[cls]), keys))
        objs = {k_cls: list(map(lambda d: k_cls(**d), data)) for k_cls, data in data_map.items()}
        for cls in keys:
            cls.objects.bulk_create(objs[cls])
        self._primitives = objs
        return objs

    def _gen_expiry_time(self):
        minutes_valid = random.randrange(0,60)
        if minutes_valid > 5:
            expiry_time = timezone.now() + timedelta(minutes=minutes_valid)
            expiry_time = expiry_time.isoformat()
        else:
            expiry_time = None
        return expiry_time

    def _gen_roles(self, num=None):
        if num is None:
            num = self.num_roles
        out = []
        if num > 0:
            roles_gen = listitem_rand_assigner(list_=self._primitives[Role], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            for role in roles_gen:
                data = {'expiry': self._gen_expiry_time(), 'role':role.id,
                        'approved_by': random.randrange(3,1000), # will NOT write this field to model
                        }
                out.append(data)
        return out


    def _gen_quota(self, num=None):
        if num is None:
            num = self.num_quota
        self.num_locations = 0
        self.num_emails = 0
        self.num_phones = 0
        out = []
        if num > 0:
            materials_gen = listitem_rand_assigner(list_=self._primitives[QuotaMaterial], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            for material in materials_gen:
                maxnum = random.randrange(1,10)
                if material.app_code == AppCodeOptions.user_management:
                    if material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value:
                        self.num_phones = maxnum
                    elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value:
                        self.num_emails = maxnum
                    elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value:
                        self.num_locations = maxnum
                data = {'expiry':self._gen_expiry_time(), 'material':material.id, 'maxnum':maxnum}
                out.append(data)
        return out


    def _gen_locations(self, num=None):
        if num is None:
            num = min(self.num_locations, len(_fixtures[GeoLocation]))
        out = []
        if num > 0:
            data_gen = listitem_rand_assigner(list_=_fixtures[GeoLocation], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            out = list(data_gen)
        return out

    def _gen_emails(self, num=None):
        if num is None:
            num = min(self.num_emails, len(_fixtures[EmailAddress]))
        out = []
        if num > 0:
            data_gen = listitem_rand_assigner(list_=_fixtures[EmailAddress], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            out = list(data_gen)
        return out


    def _gen_phones(self, num=None):
        if num is None:
            num = min(self.num_phones, len(_fixtures[PhoneNumber]))
        out = []
        if num > 0:
            data_gen = listitem_rand_assigner(list_=_fixtures[PhoneNumber], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            out = list(data_gen)
        return out


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
## end of class HttpRequestDataGenGroup


class GroupVerificationMixin:
    serializer_class = GenericUserGroupSerializer
    err_msg_loop_detected = 'will form a loop, which is NOT allowed in closure table'

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
        value = {'id': node.id, 'name': node.name}
        _fields_compare = _nested_field_names['roles']
        value['roles'] = list(node.roles.values(*_fields_compare))
        for d in value['roles']:
            if not d['expiry']:
                continue
            d['expiry'] = d['expiry'].isoformat()
        _fields_compare = _nested_field_names['quota']
        value['quota'] = list(node.quota.values(*_fields_compare))
        for d in value['quota']:
            if not d['expiry']:
                continue
            d['expiry'] = d['expiry'].isoformat()
        _fields_compare = _nested_field_names['emails']
        value['emails'] = list(node.emails.values(*_fields_compare))
        _fields_compare = _nested_field_names['phones']
        value['phones'] = list(node.phones.values(*_fields_compare))
        _fields_compare = _nested_field_names['locations']
        value['locations'] = list(node.locations.values(*_fields_compare))
        return value


    def _value_compare_fn(self, val_a, val_b):
        fields_eq = {}
        fields_eq['name'] = val_a['name'] == val_b['name']
        instance = GenericUserGroup.objects.get(id=val_b['id'])
        fields_eq['roles']  = self._value_compare_roles_fn(val_a=val_a, val_b=val_b)
        fields_eq['quota']  = self._value_compare_quota_fn(val_a=val_a, val_b=val_b)
        for k in ('emails', 'phones', 'locations'):
            fields_eq[k] = self._value_compare_contact_fn(val_a=val_a[k], val_b=val_b[k],
                    _fields_compare=_nested_field_names[k])
        return reduce(operator.and_, fields_eq.values())

    def _value_compare_roles_fn(self, val_a, val_b):
        _fields_compare = _nested_field_names['roles']
        expect_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_a['roles']))
        actual_val = val_b['roles']
        expect_val = sorted(expect_val, key=lambda d:d['role'])
        actual_val = sorted(actual_val, key=lambda d:d['role'])
        return actual_val == expect_val

    def _value_compare_quota_fn(self, val_a, val_b):
        _fields_compare = _nested_field_names['quota']
        expect_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_a['quota']))
        actual_val = val_b['quota']
        expect_val = sorted(expect_val, key=lambda d:d['material'])
        actual_val = sorted(actual_val, key=lambda d:d['material'])
        return actual_val == expect_val

    def _value_compare_contact_fn(self, val_a, val_b, _fields_compare, compare_id=False):
        if not compare_id:
            _fields_compare = _fields_compare.copy()
            _fields_compare.remove('id')
        expect_val = list(map(lambda d: tuple([d[fname] for fname in _fields_compare]), val_a))
        actual_val = list(map(lambda d: tuple([d[fname] for fname in _fields_compare]), val_b))
        expect_val = sorted(expect_val)
        actual_val = sorted(actual_val)
        return actual_val == expect_val


