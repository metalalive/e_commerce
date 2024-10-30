from django.utils.translation import gettext_lazy as _
from rest_framework.exceptions import (
    APIException,
    ValidationError as DRFValidationError,
)
from rest_framework import status as DRFstatus


class DRFRequestDataConflictError(DRFValidationError):
    status_code = DRFstatus.HTTP_409_CONFLICT
    default_detail = _("Conflict due to invalid input")
    default_code = "conflict"

