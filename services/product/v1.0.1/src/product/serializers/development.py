import logging

from ..models.development import ProductDevIngredient
from .common import BaseIngredientSerializer

_logger = logging.getLogger(__name__)


class FabricationIngredientSerializer(BaseIngredientSerializer):
    class Meta(BaseIngredientSerializer.Meta):
        model = ProductDevIngredient
        fields = ["id", "name", "category"]
