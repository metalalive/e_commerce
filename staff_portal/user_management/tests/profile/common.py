import random
import string

from user_management.models.auth import Role
from user_management.models.base import QuotaMaterial, GenericUserProfile, GenericUserGroup, GenericUserGroupClosure, EmailAddress, PhoneNumber, GeoLocation
from user_management.serializers import GenericUserProfileSerializer

from tests.python.common import listitem_rand_assigner, HttpRequestDataGen
from ..common import _fixtures, UserNestedFieldSetupMixin, UserNestedFieldVerificationMixin

_nested_field_names = {
    'groups': ['group',],
    'roles': ['expiry', 'role'],
    'quota': ['material','maxnum','expiry'],
    'locations':['id', 'country', 'province', 'locality', 'street', 'detail', 'floor', 'description'],
    'emails':['id','addr'],
    'phones':['id','line_number','country_code'],
}


class HttpRequestDataGenProfile(HttpRequestDataGen, UserNestedFieldSetupMixin):
    num_groups = 0

    @property
    def num_default_profiles(self):
        return 3

    def init_primitive(self):
        keys = (Role, QuotaMaterial, GenericUserGroup)
        data_map = dict(map(lambda cls: (cls, _fixtures[cls]), keys))
        objs = {k_cls: list(map(lambda d: k_cls(**d), data)) for k_cls, data in data_map.items()}
        for cls in keys:
            cls.objects.bulk_create(objs[cls])
        # the profiles that were already created, will be used to create another new profiles
        # through serializer or API endpoint
        default_profile_data = _fixtures[GenericUserProfile][:self.num_default_profiles]
        objs[GenericUserProfile] = list(map(lambda d: GenericUserProfile(**d), default_profile_data))
        GenericUserProfile.objects.bulk_create(objs[GenericUserProfile])
        self._primitives = objs
        return objs

    def _gen_roles(self, num=None):
        return super()._gen_roles(role_objs=self._primitives[Role] , num=num)

    def _gen_quota(self, num=None):
        return super()._gen_quota(quota_mat_objs=self._primitives[QuotaMaterial], num=num)

    def _gen_groups(self, num=None):
        if num is None:
            num = self.num_groups
        out = []
        if num > 0:
            grps_gen = listitem_rand_assigner(list_=self._primitives[GenericUserGroup], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            for grp in grps_gen:
                data = {'group': grp.id, 'approved_by': random.randrange(900,1000)}
                out.append(data)
        return out

    def customize_req_data_item(self, item, **kwargs):
        data = {fname: getattr(self, '_gen_%s' % fname)()  for fname in _nested_field_names.keys()}
        item.update(data)
## end of class HttpRequestDataGenProfile


class ProfileVerificationMixin(UserNestedFieldVerificationMixin):
    serializer_class = GenericUserProfileSerializer
    _nested_field_names = _nested_field_names

    def _load_profiles_from_instances(self, objs):
        _fields_compare = _nested_field_names['groups']
        out = []
        for obj in objs:
            data = self.load_group_from_instance(obj=obj)
            data = data | {'first_name':obj.first_name, 'last_name':obj.last_name}
            data['groups'] = list(obj.groups.values(*_fields_compare))
            out.append(data)
        return out

    def _value_compare_groups_fn(self, val_a, val_b):
        _fields_compare = self._nested_field_names['groups']
        expect_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_a['groups']))
        actual_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_b['groups']))
        expect_val = sorted(expect_val, key=lambda d:d['group'])
        actual_val = sorted(actual_val, key=lambda d:d['group'])
        return actual_val == expect_val

    def _value_compare_fn(self, val_a, val_b):
        fields_eq = super()._value_compare_fn(val_a, val_b)
        fields_eq['first_name'] = val_a['first_name'] == val_b['first_name']
        fields_eq['last_name'] = val_a['last_name'] == val_b['last_name']
        fields_eq['groups']  = self._value_compare_groups_fn(val_a=val_a, val_b=val_b)
        self.assertNotIn(False, fields_eq.values())

    def verify_data(self, actual_data, expect_data, profile):
        actual_data = self._load_profiles_from_instances(objs=actual_data)
        actual_data_iter = iter(actual_data)
        for expect_value in expect_data:
            actual_value = next(actual_data_iter)
            self._value_compare_fn(val_a=expect_value, val_b=actual_value)


