import logging

from rest_framework.serializers import ListSerializer
from rest_framework.exceptions import ValidationError as RestValidationError

_logger = logging.getLogger(__name__)


class NestedFieldSetupMixin:
    def _setup_subform_instance(self, name, instance, data, pk_field_name="id"):
        field = self.fields[name]
        field.instance = None
        if instance is None:
            return
        mgt = getattr(instance, name)
        log_msg = ["subform_name", name, "pk_field_name", pk_field_name]
        if isinstance(field, ListSerializer):
            raw_subform = data.get(name, [])
            try:
                if isinstance(pk_field_name, str):
                    ids = [
                        d[pk_field_name] for d in raw_subform if d.get(pk_field_name)
                    ]
                elif isinstance(pk_field_name, (list, tuple)):  # composite key
                    ids = [
                        tuple(d[c] for c in pk_field_name if d.get(c))
                        for d in raw_subform
                    ]
                    # raise NotImplementedError()
                log_msg += ["IDs", ids]
                if mgt:
                    field.instance = mgt.filter(pk__in=ids)
            except (TypeError, AttributeError) as e:
                _logger.warning(None, "exception", str(e), *log_msg)
                errmsg = {name: ["improper data format"]}
                raise RestValidationError(errmsg)
            except (Exception,) as e:
                # fmt: off
                log_msg.extend([
                    "instance_cls", type(instance).__name__, "data_cls", type(data).__name__,
                    "raw_subform", str(raw_subform), type(e).__name__, str(e),
                ])
                # fmt: on
                _logger.error(None, *log_msg)
                raise e
        else:
            field.instance = mgt
            log_msg += ["ID", field.instance.pk]
        _logger.debug(None, *log_msg, stacklevel=1)

    def _mark_as_creation_on_update(self, pk_field_name, instance, data):
        """
        it is possible to craate new subform item(s) during bulk update operation
        """
        # note that ExtendModelSerializer internally changes the id field to Integer Field if data
        # argument is given on initialization.
        # this function works only for the instance which has auto-increment id field
        # , TODO: rename function name
        pk_field = self.fields[pk_field_name]
        if not pk_field.read_only:
            if instance is None:
                data.pop(pk_field_name, None)
                pk_field.required = False
            else:
                pk_field.required = True
