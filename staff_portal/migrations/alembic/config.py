from pathlib import Path
from alembic.config import Config

from common.util.python import format_sqlalchemy_url

class ExtendedConfig(Config):
    def __init__(self, *args, template_base_path:Path=None, **kwargs):
        condition = template_base_path and template_base_path.exists() and template_base_path.is_dir()
        assert condition , 'should be existing path to custom template'
        self._template_base_path = template_base_path
        super().__init__(*args, **kwargs)

    def get_template_directory(self) -> str:
        return str(self._template_base_path)

    def set_url(self, db_credential, driver_label):
        url = format_sqlalchemy_url(driver=driver_label, db_credential=db_credential)
        self.set_main_option(name='sqlalchemy.url', value=url)

