import logging
import traceback
from blacksheep import Router, Response, TextContent
from guardpost.authorization import UnauthorizedError, ForbiddenError

router = Router()

from .tag import TagController as TagController  # noqa: E402
from .attribute_label import AttrLabelController as AttrLabelController  # noqa: E402
from .saleable import SaleItemController as SaleItemController  # noqa: E402

_logger = logging.getLogger(__name__)


def _format_full_exception(exc: BaseException) -> str:
    """
    Return the full formatted exception + cause/context tracebacks (if any).
    """
    parts = []
    parts.append("Main exception:\n")
    parts.append("".join(traceback.format_exception(type(exc), exc, exc.__traceback__)))

    # __cause__ is explicit 'raise ... from ...'
    cause = getattr(exc, "__cause__", None)
    if cause:
        parts.append("\nCause exception:\n")
        parts.append("".join(traceback.format_exception(type(cause), cause, cause.__traceback__)))

    # __context__ is implicit context when exception was raised during handling another
    context = getattr(exc, "__context__", None)
    if context and context is not cause:
        parts.append("\nContext exception:\n")
        parts.append(
            "".join(traceback.format_exception(type(context), context, context.__traceback__))
        )
    return "".join(parts)


async def unauthorized_handler(app_obj, request, exc: UnauthorizedError):
    # log verbose info (method/path headers limited to avoid leaking secrets)
    if _logger.getEffectiveLevel() == logging.DEBUG:
        try:
            path = getattr(request, "path", None) or getattr(request, "uri", "<unknown>")
            method = getattr(request, "method", "<unknown>")
            _logger.debug("UnauthorizedError while handling %s %s: %s", method, path, exc)
            loginuser = getattr(request, "user", None)
            isauthed = getattr(loginuser, "is_authenticated", lambda: None)()
            authmode = getattr(loginuser, "authentication_mode", None)
            _logger.debug(
                "user-id %s , mode: %s, authed: %s", str(loginuser), authmode, str(isauthed)
            )
        except Exception:
            # best-effort metadata, do not fail logging
            _logger.exception("UnauthorizedError (failed to read request metadata)")

        # log full formatted traceback + causes
        _logger.debug(_format_full_exception(exc))

    # Keep response content generic for security.
    return Response(401, content=TextContent("Unauthorized"))


async def forbidden_handler(app_obj, request, exc: ForbiddenError):
    if _logger.getEffectiveLevel() == logging.DEBUG:
        try:
            path = getattr(request, "path", None) or getattr(request, "uri", "<unknown>")
            method = getattr(request, "method", "<unknown>")
            _logger.debug("ForbiddenError while handling %s %s: %s", method, path, exc)
        except Exception:
            _logger.exception("ForbiddenError (failed to read request metadata)")

        _logger.debug(_format_full_exception(exc))
    return Response(403, content=TextContent("Forbidden"))
