from django.contrib.contenttypes.models  import ContentType

from common.serializers  import  ExtendedModelSerializer
from common.serializers.mixins  import  NestedFieldSetupMixin

from ..models import GenericUserGroup, GenericUserProfile


class ConnectedGroupField(ExtendedModelSerializer):
    class Meta(ExtendedModelSerializer.Meta):
        model = GenericUserGroup
        fields = ['id', 'name']
        read_only_fields = ['name']

class ConnectedProfileField(ExtendedModelSerializer):
    class Meta(ExtendedModelSerializer.Meta):
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


