import random

from user_management.models.auth import Role
from user_management.models.base import (
    QuotaMaterial,
    GenericUserProfile,
    GenericUserGroup,
    GenericUserGroupClosure,
)
from user_management.serializers import GenericUserProfileSerializer

from ecommerce_common.tests.common import listitem_rand_assigner, HttpRequestDataGen
from ..common import (
    _fixtures,
    UserNestedFieldSetupMixin,
    UserNestedFieldVerificationMixin,
)

_nested_field_names = {
    "groups": [
        "group",
    ],
    "roles": ["expiry", "role"],
    "quota": ["material", "maxnum", "expiry"],
    "locations": [
        "id",
        "country",
        "province",
        "locality",
        "street",
        "detail",
        "floor",
        "description",
    ],
    "emails": ["id", "addr"],
    "phones": ["id", "line_number", "country_code"],
}


class HttpRequestDataGenProfile(HttpRequestDataGen, UserNestedFieldSetupMixin):
    num_groups = 0

    @property
    def num_default_profiles(self):
        return 3

    def init_primitive(self):
        keys = (Role, QuotaMaterial, GenericUserGroup)
        data_map = dict(map(lambda cls: (cls, _fixtures[cls]), keys))
        objs = {
            k_cls: list(map(lambda d: k_cls(**d), data))
            for k_cls, data in data_map.items()
        }
        for cls in keys:
            cls.objects.bulk_create(objs[cls])
        # the profiles that were already created, will be used to create another new profiles
        # through serializer or API endpoint
        default_profile_data = _fixtures[GenericUserProfile][
            : self.num_default_profiles
        ]
        objs[GenericUserProfile] = list(
            map(lambda d: GenericUserProfile(**d), default_profile_data)
        )
        GenericUserProfile.objects.bulk_create(objs[GenericUserProfile])
        self._primitives = objs
        return objs

    def _setup_groups_hierarchy(self):
        # the tree structure of the group hierarchy in this test file
        #
        #            3             8         11
        #          /    \        /  \       /  \
        #         4      5      9   10     12   13
        #        / \                           /
        #       6   7                         14

        grp_obj_map = dict(
            map(lambda obj: (obj.id, obj), self._primitives[GenericUserGroup])
        )
        group_closure_data = [
            {
                "id": 1,
                "depth": 0,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[3],
            },
            {
                "id": 2,
                "depth": 0,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[4],
            },
            {
                "id": 3,
                "depth": 0,
                "ancestor": grp_obj_map[5],
                "descendant": grp_obj_map[5],
            },
            {
                "id": 4,
                "depth": 0,
                "ancestor": grp_obj_map[6],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 5,
                "depth": 0,
                "ancestor": grp_obj_map[7],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 6,
                "depth": 1,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[4],
            },
            {
                "id": 7,
                "depth": 1,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[5],
            },
            {
                "id": 8,
                "depth": 1,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 9,
                "depth": 1,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 10,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 11,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[7],
            },
            # ---------
            {
                "id": 12,
                "depth": 0,
                "ancestor": grp_obj_map[8],
                "descendant": grp_obj_map[8],
            },
            {
                "id": 13,
                "depth": 0,
                "ancestor": grp_obj_map[9],
                "descendant": grp_obj_map[9],
            },
            {
                "id": 14,
                "depth": 0,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[10],
            },
            {
                "id": 15,
                "depth": 1,
                "ancestor": grp_obj_map[8],
                "descendant": grp_obj_map[9],
            },
            {
                "id": 16,
                "depth": 1,
                "ancestor": grp_obj_map[8],
                "descendant": grp_obj_map[10],
            },
            # ---------
            {
                "id": 17,
                "depth": 0,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[11],
            },
            {
                "id": 18,
                "depth": 0,
                "ancestor": grp_obj_map[12],
                "descendant": grp_obj_map[12],
            },
            {
                "id": 19,
                "depth": 0,
                "ancestor": grp_obj_map[13],
                "descendant": grp_obj_map[13],
            },
            {
                "id": 20,
                "depth": 0,
                "ancestor": grp_obj_map[14],
                "descendant": grp_obj_map[14],
            },
            {
                "id": 21,
                "depth": 1,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[12],
            },
            {
                "id": 22,
                "depth": 1,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[13],
            },
            {
                "id": 23,
                "depth": 1,
                "ancestor": grp_obj_map[13],
                "descendant": grp_obj_map[14],
            },
            {
                "id": 24,
                "depth": 2,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[14],
            },
        ]
        list(
            map(
                lambda d: GenericUserGroupClosure.objects.create(**d),
                group_closure_data,
            )
        )
        return grp_obj_map

    def _refresh_applied_groups(self, profile, groups, approved_by=None):
        approved_by = approved_by or self._primitives[GenericUserProfile][1]
        profile.groups.all(with_deleted=True).delete(hard=True)
        for grp_obj in groups:
            profile.groups.create(group=grp_obj, approved_by=approved_by)

    def _gen_roles(self, num=None):
        return super()._gen_roles(role_objs=self._primitives[Role], num=num)

    def _gen_quota(self, num=None):
        return super()._gen_quota(
            quota_mat_objs=self._primitives[QuotaMaterial], num=num
        )

    def _gen_groups(self, num=None):
        if num is None:
            num = self.num_groups
        out = []
        if num > 0:
            grps_gen = listitem_rand_assigner(
                list_=self._primitives[GenericUserGroup],
                min_num_chosen=num,
                max_num_chosen=(num + 1),
            )
            for grp in grps_gen:
                data = {"group": grp.id, "approved_by": random.randrange(900, 1000)}
                out.append(data)
        return out

    def customize_req_data_item(self, item, **kwargs):
        data = {
            fname: getattr(self, "_gen_%s" % fname)()
            for fname in _nested_field_names.keys()
        }
        item.update(data)


## end of class HttpRequestDataGenProfile


class ProfileVerificationMixin(UserNestedFieldVerificationMixin):
    serializer_class = GenericUserProfileSerializer
    _nested_field_names = _nested_field_names

    def _load_profiles_from_instances(self, objs):
        _fields_compare = _nested_field_names["groups"]
        out = []
        for obj in objs:
            data = self.load_group_from_instance(obj=obj)
            data = data | {"first_name": obj.first_name, "last_name": obj.last_name}
            data["groups"] = list(obj.groups.values(*_fields_compare))
            out.append(data)
        return out

    def _value_compare_groups_fn(self, val_a, val_b):
        _fields_compare = self._nested_field_names["groups"]
        expect_val = list(
            map(
                lambda d: {fname: d[fname] for fname in _fields_compare},
                val_a["groups"],
            )
        )
        actual_val = list(
            map(
                lambda d: {fname: d[fname] for fname in _fields_compare},
                val_b["groups"],
            )
        )
        expect_val = sorted(expect_val, key=lambda d: d["group"])
        actual_val = sorted(actual_val, key=lambda d: d["group"])
        return actual_val == expect_val

    def _value_compare_fn(self, val_a, val_b):
        fields_eq = super()._value_compare_fn(val_a, val_b)
        fields_eq["first_name"] = val_a["first_name"] == val_b["first_name"]
        fields_eq["last_name"] = val_a["last_name"] == val_b["last_name"]
        fields_eq["groups"] = self._value_compare_groups_fn(val_a=val_a, val_b=val_b)
        self.assertNotIn(False, fields_eq.values())

    def verify_data(self, actual_data, expect_data):
        actual_data = self._load_profiles_from_instances(objs=actual_data)
        actual_data_iter = iter(actual_data)
        for expect_value in expect_data:
            actual_value = next(actual_data_iter)
            self._value_compare_fn(val_a=expect_value, val_b=actual_value)
