

class BaseGetProfileMixin:
    UNKNOWN_ID = -1

    def get_account(self, **kwargs):
        raise NotImplementedError()

    def get_account_id(self, **kwargs):
        raise NotImplementedError()

    def get_profile_id(self, **kwargs):
        raise NotImplementedError()

    def get_profile(self, **kwargs):
        raise NotImplementedError()

