from django.contrib.contenttypes.models import ContentType
from rest_framework.serializers import ModelSerializer

from ecommerce_common.serializers.mixins import NestedFieldSetupMixin
from ecommerce_common.models.enums.django import AppCodeOptions

from ..models.base import GenericUserGroup, GenericUserProfile, QuotaMaterial


class ConnectedGroupField(ModelSerializer):
    class Meta:
        model = GenericUserGroup
        fields = ["id", "name"]
        read_only_fields = ["id", "name"]


class ConnectedProfileField(ModelSerializer):
    class Meta:
        model = GenericUserProfile
        fields = ["id", "first_name", "last_name"]
        read_only_fields = ["id", "first_name", "last_name"]


class UserSubformSetupMixin(NestedFieldSetupMixin):
    def _append_user_field(self, name, instance, data):
        # append user field to data
        # instance must be user group or user profile
        # the 2 fields are never modified by client data
        if instance and instance.pk:
            for d in data.get(name, []):
                d["user_type"] = ContentType.objects.get_for_model(instance).pk
                d["user_id"] = instance.pk


#### end of UserSubformSetupMixin


def serialize_profile_quota(profile, app_labels):
    try:
        mat_qset = QuotaMaterial.get_for_apps(app_labels=app_labels)
    except AttributeError as e:
        err_msg = "receive invalid app_label %s" % e.args[0]
        raise ValueError(err_msg)
    mat_qset = mat_qset.values("id", "app_code", "mat_code")
    quota_mat_map = dict(map(lambda d: (d["id"], d), mat_qset))
    fetch_mat_ids = quota_mat_map.keys()
    all_quota = profile.all_quota
    filtered_quota = filter(lambda kv: kv[0] in fetch_mat_ids, all_quota.items())
    filtered_quota = map(
        lambda kv: {
            "app_code": quota_mat_map[kv[0]]["app_code"],
            "mat_code": quota_mat_map[kv[0]]["mat_code"],
            "maxnum": kv[1],
        },
        filtered_quota,
    )
    return list(filtered_quota)


def serialize_profile_permissions(profile, app_labels):
    out = []
    all_roles = profile.all_roles
    role_types = ("direct", "inherit")
    for role_type in role_types:
        perm_qset = all_roles[role_type].get_permissions(app_labels=app_labels)
        vals = perm_qset.values_list("content_type__app_label", "codename")
        vals = filter(lambda d: getattr(AppCodeOptions, d[0], None), vals)
        vals = map(
            lambda d: {
                "app_code": getattr(AppCodeOptions, d[0]).value,
                "codename": d[1],
            },
            vals,
        )
        out.extend(vals)
    return out
