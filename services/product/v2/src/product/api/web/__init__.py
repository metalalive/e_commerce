from blacksheep import Router

router = Router()

from .tag import TagController as TagController  # noqa: E402
from .attribute_label import AttrLabelController as AttrLabelController  # noqa: E402
