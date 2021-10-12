from django.contrib.contenttypes.models  import ContentType
from rest_framework.serializers import ModelSerializer
from common.serializers.mixins  import  NestedFieldSetupMixin

from ..models.base import GenericUserGroup, GenericUserProfile


class ConnectedGroupField(ModelSerializer):
    class Meta:
        model = GenericUserGroup
        fields = ['id', 'name']
        read_only_fields = ['id','name']

class ConnectedProfileField(ModelSerializer):
    class Meta:
        model = GenericUserProfile
        fields = ['id', 'first_name', 'last_name']
        read_only_fields = ['id', 'first_name', 'last_name']


class UserSubformSetupMixin(NestedFieldSetupMixin):
    def _append_user_field(self, name, instance, data):
        # append user field to data
        # instance must be user group or user profile
        # the 2 fields are never modified by client data
        if instance and instance.pk:
            for d in data.get(name, []):
                d['user_type'] = ContentType.objects.get_for_model(instance).pk
                d['user_id'] = instance.pk

#### end of UserSubformSetupMixin


