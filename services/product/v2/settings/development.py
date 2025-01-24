from .common import *  # noqa: F403
from .celeryconfig import *  # noqa: F403

DATABASES["confidential_path"] = (  # noqa: F405
    "backend_apps.databases.product_dev_service_v2"
)
