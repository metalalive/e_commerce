import os
import sys
import logging

sys.path.insert(0, os.path.abspath("./src"))
sys.path.insert(0, os.path.abspath("."))
os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings.development")
import django  # noqa : E402

try:
    django.setup()
except ImportError as e:
    sys.stderr.write(
        "Error: Django setup failed. Ensure DJANGO_SETTINGS_MODULE is set correctly "
        "and the 'src' directory is in the Python path.\n"
        f"Details: {e}\n"
    )
    sys.exit(1)


root_logger = logging.getLogger()
if root_logger.handlers:
    for handler in root_logger.handlers:
        root_logger.removeHandler(handler)

logging.basicConfig(
    level=logging.DEBUG,
    format="%(asctime)s - %(levelname)s - %(message)s",
    stream=sys.stdout,
)

import devtools as devtools  # noqa : E402
