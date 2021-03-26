from django.db import models
from django.utils.translation import gettext_lazy as _

class CompoundPrimaryKeyField(models.Field):
    empty_strings_allowed = False
    default_error_messages = {
        'invalid': _('“%(value)s” value must be a set of existing fields.'),
    }
    description = _("CompoundPrimaryKey")

    def __init__(self, inc_fields, **kwargs):
        assert any(inc_fields), '`fields` argument has to be non-empty list'
        self._inc_fields_name = inc_fields
        kwargs['primary_key'] = True
        kwargs['auto_created'] = False
        kwargs['editable'] = False
        kwargs.pop('db_column', None)
        super().__init__(**kwargs)

    def get_internal_type(self):
        # fixed class name for all subclasses
        return "CompoundPrimaryKeyField"

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        kwargs['inc_fields'] = self._inc_fields_name
        return name, path, args, kwargs

    def db_type(self, connection):
        """
        return data type of column in database table
        """
        # this field returns any value other than `None` to skip check
        # when creating database table
        return ''

    def db_check(self, connection):
        # return extra check of SQL clause on this field
        pass

    @property
    def db_columns(self):
        if not self.db_column:
            self.db_column = self._composite_columns()
        return self.db_column

    def get_attname_column(self):
        if not self.db_column:
            self.db_column = self._composite_columns()
        return  super().get_attname_column()

    def _composite_columns(self):
        if not hasattr(self,'model'):
            return
        # self.model might not exist ??
        out = []
        for fd in self.model._meta.local_fields:
            if isinstance(fd, type(self)):
                continue
            if fd.name in self._inc_fields_name:
                out.append(fd.db_column)
        return out


