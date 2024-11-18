import os
from importlib import import_module

import pytest


@pytest.fixture(scope="session")
def app_setting():
    app_setting_path = os.environ["APP_SETTINGS"]
    setting = import_module(app_setting_path)
    yield setting
