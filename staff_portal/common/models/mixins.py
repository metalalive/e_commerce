
class MinimumInfoMixin:
    """
    callers can simply run the function minimum_info to retrieve minimal information
    without knowing exact field names for representation purpose.
    Subclasses can add few more the field values by overriding this function.
    """
    @property
    def minimum_info(self):
        if not hasattr(self, 'min_info_field_names'):
            raise NotImplementedError
        field_names = getattr(self, 'min_info_field_names')
        return {fname: getattr(self, fname) for fname in field_names}


class SerializableMixin:
    def serializable(self, present, query_fn):
        out = {}
        present = present or []
        for field_name in present:
            if not isinstance(field_name, (str,)):
                continue
            fd_value = getattr(self, field_name, None)
            if fd_value:
                if isinstance(fd_value, (str, int, float, bool)):
                    out[field_name] = fd_value
                else:
                    query_fn(fd_value=fd_value, field_name=field_name, out=out)
        return out


