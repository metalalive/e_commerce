
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


