import json
from pydantic import BaseSettings

_valid_attrs = ['keystore_config', 'secrets_file_path', 'apps']

class AppSettings(BaseSettings):
    fastapi_config_filepath:str = "/path/to/fastapi/config.json"

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        #import pdb
        #pdb.set_trace()

    def _get_config_attr(self, key):
        out = None
        with open(self.fastapi_config_filepath) as f:
            jf = json.load(f)
            out = jf[key]
        return out

    def __getattr__(self, key):
        if key in _valid_attrs:
            out = self._get_config_attr(key)
        else:
            out = super().__getattr__(key)
        return out


settings = AppSettings()

